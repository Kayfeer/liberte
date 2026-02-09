//! High-level message send/receive helpers over GossipSub topics.
//!
//! Provides ergonomic functions for subscribing to channel topics and
//! publishing encrypted messages, bridging the gap between the application
//! layer (which deals with `WireMessage` and encryption) and the swarm
//! command/notification channels.

use tokio::sync::mpsc;
use tracing::{debug, error};

use liberte_shared::crypto::{decrypt, encrypt, SymmetricKey};
use liberte_shared::protocol::WireMessage;
use liberte_shared::types::ChannelId;

use crate::swarm::{SwarmCommand, SwarmNotification};

/// Subscribe to a channel's GossipSub topic.
///
/// The topic name is derived from the channel ID via [`ChannelId::to_topic`].
///
/// # Arguments
///
/// * `cmd_tx` - The swarm command channel sender
/// * `channel_id` - The channel to subscribe to
pub async fn subscribe_topic(
    cmd_tx: &mpsc::Sender<SwarmCommand>,
    channel_id: &ChannelId,
) -> anyhow::Result<()> {
    let topic = channel_id.to_topic();
    debug!(topic = %topic, "Subscribing to channel topic");

    cmd_tx
        .send(SwarmCommand::SubscribeTopic(topic))
        .await
        .map_err(|_| anyhow::anyhow!("Swarm command channel closed"))?;

    Ok(())
}

/// Publish an encrypted `WireMessage` to a channel's GossipSub topic.
///
/// The message is first serialized to bincode, then encrypted with the
/// channel's symmetric key (XChaCha20-Poly1305), and finally published
/// on the appropriate GossipSub topic.
///
/// # Arguments
///
/// * `cmd_tx` - The swarm command channel sender
/// * `channel_id` - The target channel
/// * `channel_key` - The symmetric encryption key for this channel
/// * `message` - The wire message to publish
pub async fn publish_message(
    cmd_tx: &mpsc::Sender<SwarmCommand>,
    channel_id: &ChannelId,
    channel_key: &SymmetricKey,
    message: &WireMessage,
) -> anyhow::Result<()> {
    let topic = channel_id.to_topic();

    // Serialize to bincode
    let plaintext = message
        .to_bytes()
        .map_err(|e| anyhow::anyhow!("Serialization error: {e}"))?;

    // Encrypt with the channel key
    let ciphertext =
        encrypt(channel_key, &plaintext).map_err(|e| anyhow::anyhow!("Encryption error: {e}"))?;

    debug!(
        topic = %topic,
        plaintext_len = plaintext.len(),
        ciphertext_len = ciphertext.len(),
        "Publishing encrypted message"
    );

    cmd_tx
        .send(SwarmCommand::PublishMessage {
            topic,
            data: ciphertext,
        })
        .await
        .map_err(|_| anyhow::anyhow!("Swarm command channel closed"))?;

    Ok(())
}

/// Attempt to decrypt and deserialize a received GossipSub message payload.
///
/// # Arguments
///
/// * `data` - The raw ciphertext received from GossipSub
/// * `channel_key` - The symmetric key for the channel
///
/// # Returns
///
/// The deserialized `WireMessage` on success, or an error if decryption or
/// deserialization fails (e.g. wrong key, corrupted data).
pub fn decode_message(data: &[u8], channel_key: &SymmetricKey) -> anyhow::Result<WireMessage> {
    let plaintext =
        decrypt(channel_key, data).map_err(|e| anyhow::anyhow!("Decryption error: {e}"))?;

    let message = WireMessage::from_bytes(&plaintext)
        .map_err(|e| anyhow::anyhow!("Deserialization error: {e}"))?;

    Ok(message)
}

/// Process incoming notifications looking for messages on a specific topic.
///
/// This is a convenience filter that checks each notification and, if it is
/// a `MessageReceived` on the given topic, tries to decrypt and return it.
///
/// # Arguments
///
/// * `notification` - A swarm notification
/// * `channel_id` - The channel ID to filter for
/// * `channel_key` - The encryption key for the channel
///
/// # Returns
///
/// `Some(WireMessage)` if the notification is a message on the target topic
/// that decrypts successfully, `None` otherwise.
pub fn try_decode_notification(
    notification: &SwarmNotification,
    channel_id: &ChannelId,
    channel_key: &SymmetricKey,
) -> Option<WireMessage> {
    match notification {
        SwarmNotification::MessageReceived {
            topic, data, ..
        } => {
            let expected_topic = channel_id.to_topic();
            if topic != &expected_topic {
                return None;
            }

            match decode_message(data, channel_key) {
                Ok(msg) => Some(msg),
                Err(e) => {
                    error!(
                        topic = %topic,
                        error = %e,
                        "Failed to decode message on channel"
                    );
                    None
                }
            }
        }
        _ => None,
    }
}
