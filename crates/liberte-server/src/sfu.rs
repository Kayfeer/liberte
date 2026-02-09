use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use libp2p::PeerId;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Opaque encrypted media frame -- the SFU never decrypts this.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct EncryptedFrame {
    pub sender: PeerId,
    pub payload: Vec<u8>,
}

#[allow(dead_code)]
pub struct SfuRoom {
    pub room_id: Uuid,
    participants: HashSet<PeerId>,
    senders: HashMap<PeerId, mpsc::Sender<EncryptedFrame>>,
}

#[allow(dead_code)]
impl SfuRoom {
    pub fn new(room_id: Uuid) -> Self {
        Self {
            room_id,
            participants: HashSet::new(),
            senders: HashMap::new(),
        }
    }

    pub fn join(&mut self, peer_id: PeerId) -> mpsc::Receiver<EncryptedFrame> {
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

    /// Forward an encrypted frame to everyone except the sender.
    pub async fn route_frame(&self, frame: EncryptedFrame) {
        let sender = frame.sender;

        for (peer_id, tx) in &self.senders {
            if *peer_id == sender {
                continue;
            }

            if tx.try_send(frame.clone()).is_err() {
                debug!(
                    room = %self.room_id,
                    target = %peer_id,
                    "Dropping frame for slow participant"
                );
            }
        }
    }

    pub fn participants(&self) -> &HashSet<PeerId> {
        &self.participants
    }

    pub fn participant_count(&self) -> usize {
        self.participants.len()
    }

    pub fn is_empty(&self) -> bool {
        self.participants.is_empty()
    }
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct SfuManager {
    rooms: Arc<RwLock<HashMap<Uuid, SfuRoom>>>,
}

#[allow(dead_code)]
impl SfuManager {
    pub fn new() -> Self {
        Self {
            rooms: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn create_room(&self) -> Uuid {
        let room_id = Uuid::new_v4();
        let room = SfuRoom::new(room_id);
        self.rooms.write().await.insert(room_id, room);
        info!(room = %room_id, "Created SFU room");
        room_id
    }

    /// Join a room (creates it if missing). Returns the frame receiver.
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

    /// Leave a room. Auto-deletes the room if it becomes empty.
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

    pub async fn route_frame(&self, room_id: &Uuid, frame: EncryptedFrame) {
        let rooms = self.rooms.read().await;
        if let Some(room) = rooms.get(room_id) {
            room.route_frame(frame).await;
        } else {
            warn!(room = %room_id, "Attempted to route frame in non-existent room");
        }
    }

    pub async fn list_rooms(&self) -> Vec<Uuid> {
        self.rooms.read().await.keys().copied().collect()
    }

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

        let received = receiver_rx.try_recv().unwrap();
        assert_eq!(received.payload, vec![0xDE, 0xAD, 0xBE, 0xEF]);
    }
}
