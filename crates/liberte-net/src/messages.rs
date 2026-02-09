use tokio::sync::mpsc;
use tracing::{debug, error};

use liberte_shared::crypto::{decrypt, encrypt, SymmetricKey};
use liberte_shared::protocol::WireMessage;
use liberte_shared::types::ChannelId;

use crate::swarm::{SwarmCommand, SwarmNotification};

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

pub async fn publish_message(
    cmd_tx: &mpsc::Sender<SwarmCommand>,
    channel_id: &ChannelId,
    channel_key: &SymmetricKey,
    message: &WireMessage,
) -> anyhow::Result<()> {
    let topic = channel_id.to_topic();

    let plaintext = message
        .to_bytes()
        .map_err(|e| anyhow::anyhow!("Serialization error: {e}"))?;

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

pub fn decode_message(data: &[u8], channel_key: &SymmetricKey) -> anyhow::Result<WireMessage> {
    let plaintext =
        decrypt(channel_key, data).map_err(|e| anyhow::anyhow!("Decryption error: {e}"))?;

    let message = WireMessage::from_bytes(&plaintext)
        .map_err(|e| anyhow::anyhow!("Deserialization error: {e}"))?;

    Ok(message)
}

/// Checks if a notification is a message on the given channel, and decrypts it if so.
pub fn try_decode_notification(
    notification: &SwarmNotification,
    channel_id: &ChannelId,
    channel_key: &SymmetricKey,
) -> Option<WireMessage> {
    match notification {
        SwarmNotification::MessageReceived { topic, data, .. } => {
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
