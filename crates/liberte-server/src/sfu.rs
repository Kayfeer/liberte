//! SFU (Selective Forwarding Unit) room management.
//!
//! The SFU routes encrypted media frames between participants of a voice/video
//! room. **It never decrypts the frames** -- participants use end-to-end
//! encryption (Insertable Streams / SFrame) so the server only sees opaque
//! ciphertext.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use libp2p::PeerId;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Encrypted frame
// ---------------------------------------------------------------------------

/// An opaque encrypted media frame.
///
/// The SFU does not inspect or decrypt the payload -- it forwards it
/// as-is to every other participant in the room.
#[derive(Debug, Clone)]
pub struct EncryptedFrame {
    /// Which participant sent this frame.
    pub sender: PeerId,
    /// Raw encrypted frame bytes (SFrame / Insertable Streams ciphertext).
    pub payload: Vec<u8>,
}

// ---------------------------------------------------------------------------
// SFU Room
// ---------------------------------------------------------------------------

/// A single voice/video room managed by the SFU.
pub struct SfuRoom {
    /// Unique room identifier.
    pub room_id: Uuid,
    /// Set of currently joined participants.
    participants: HashSet<PeerId>,
    /// Per-participant outbound channel for forwarding frames.
    /// Each participant polls their receiver to get frames from other peers.
    senders: HashMap<PeerId, mpsc::Sender<EncryptedFrame>>,
}

impl SfuRoom {
    /// Create a new, empty room.
    pub fn new(room_id: Uuid) -> Self {
        Self {
            room_id,
            participants: HashSet::new(),
            senders: HashMap::new(),
        }
    }

    /// Add a participant to the room.
    ///
    /// Returns an `mpsc::Receiver` that the participant should poll to
    /// receive forwarded frames from other participants.
    pub fn join(&mut self, peer_id: PeerId) -> mpsc::Receiver<EncryptedFrame> {
        // Bounded channel -- if a participant is slow, frames are dropped.
        let (tx, rx) = mpsc::channel::<EncryptedFrame>(256);
        self.participants.insert(peer_id);
        self.senders.insert(peer_id, tx);

        info!(
            room = %self.room_id,
            peer = %peer_id,
            participants = self.participants.len(),
            "Participant joined SFU room"
        );

        rx
    }

    /// Remove a participant from the room.
    pub fn leave(&mut self, peer_id: &PeerId) {
        self.participants.remove(peer_id);
        self.senders.remove(peer_id);

        info!(
            room = %self.room_id,
            peer = %peer_id,
            participants = self.participants.len(),
            "Participant left SFU room"
        );
    }

    /// Route an encrypted frame from `sender` to every other participant.
    ///
    /// The SFU **never** decrypts the frame -- it forwards the opaque
    /// ciphertext as-is.
    pub async fn route_frame(&self, frame: EncryptedFrame) {
        let sender = frame.sender;

        for (peer_id, tx) in &self.senders {
            // Do not echo back to the sender.
            if *peer_id == sender {
                continue;
            }

            if let Err(_) = tx.try_send(frame.clone()) {
                debug!(
                    room = %self.room_id,
                    target = %peer_id,
                    "Dropping frame for slow participant"
                );
            }
        }
    }

    /// Returns the set of currently joined participants.
    pub fn participants(&self) -> &HashSet<PeerId> {
        &self.participants
    }

    /// Returns how many participants are in the room.
    pub fn participant_count(&self) -> usize {
        self.participants.len()
    }

    /// Returns `true` if the room has no participants.
    pub fn is_empty(&self) -> bool {
        self.participants.is_empty()
    }
}

// ---------------------------------------------------------------------------
// SFU Manager
// ---------------------------------------------------------------------------

/// Manages multiple SFU rooms.
///
/// Thread-safe via `Arc<RwLock<..>>` interior -- callers obtain a handle
/// via `clone()` on the `Arc`.
#[derive(Clone)]
pub struct SfuManager {
    rooms: Arc<RwLock<HashMap<Uuid, SfuRoom>>>,
}

impl SfuManager {
    /// Create a new, empty SFU manager.
    pub fn new() -> Self {
        Self {
            rooms: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new room and return its id.
    pub async fn create_room(&self) -> Uuid {
        let room_id = Uuid::new_v4();
        let room = SfuRoom::new(room_id);
        self.rooms.write().await.insert(room_id, room);
        info!(room = %room_id, "Created SFU room");
        room_id
    }

    /// Join a peer to a room. Creates the room if it does not exist.
    ///
    /// Returns the receiver for incoming frames.
    pub async fn join_room(
        &self,
        room_id: Uuid,
        peer_id: PeerId,
    ) -> mpsc::Receiver<EncryptedFrame> {
        let mut rooms = self.rooms.write().await;
        let room = rooms
            .entry(room_id)
            .or_insert_with(|| SfuRoom::new(room_id));
        room.join(peer_id)
    }

    /// Remove a peer from a room.
    ///
    /// If the room becomes empty it is automatically deleted.
    pub async fn leave_room(&self, room_id: &Uuid, peer_id: &PeerId) {
        let mut rooms = self.rooms.write().await;
        let should_remove = if let Some(room) = rooms.get_mut(room_id) {
            room.leave(peer_id);
            room.is_empty()
        } else {
            false
        };

        if should_remove {
            rooms.remove(room_id);
            info!(room = %room_id, "Removed empty SFU room");
        }
    }

    /// Route an encrypted frame within a room.
    pub async fn route_frame(&self, room_id: &Uuid, frame: EncryptedFrame) {
        let rooms = self.rooms.read().await;
        if let Some(room) = rooms.get(room_id) {
            room.route_frame(frame).await;
        } else {
            warn!(room = %room_id, "Attempted to route frame in non-existent room");
        }
    }

    /// List all active room ids.
    pub async fn list_rooms(&self) -> Vec<Uuid> {
        self.rooms.read().await.keys().copied().collect()
    }

    /// Get the participant count for a room.
    pub async fn participant_count(&self, room_id: &Uuid) -> usize {
        self.rooms
            .read()
            .await
            .get(room_id)
            .map(|r| r.participant_count())
            .unwrap_or(0)
    }
}

impl Default for SfuManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_room_join_leave() {
        let manager = SfuManager::new();
        let room_id = manager.create_room().await;
        let peer = PeerId::random();

        let _rx = manager.join_room(room_id, peer).await;
        assert_eq!(manager.participant_count(&room_id).await, 1);

        manager.leave_room(&room_id, &peer).await;
        // Room should be auto-deleted since it is empty.
        assert_eq!(manager.list_rooms().await.len(), 0);
    }

    #[tokio::test]
    async fn test_frame_routing() {
        let manager = SfuManager::new();
        let room_id = manager.create_room().await;

        let sender = PeerId::random();
        let receiver = PeerId::random();

        let _sender_rx = manager.join_room(room_id, sender).await;
        let mut receiver_rx = manager.join_room(room_id, receiver).await;

        let frame = EncryptedFrame {
            sender,
            payload: vec![0xDE, 0xAD, 0xBE, 0xEF],
        };

        manager.route_frame(&room_id, frame.clone()).await;

        // The receiver should get the frame.
        let received = receiver_rx.try_recv().unwrap();
        assert_eq!(received.payload, vec![0xDE, 0xAD, 0xBE, 0xEF]);
    }
}
