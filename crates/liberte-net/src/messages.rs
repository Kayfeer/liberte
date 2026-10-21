use tokio::sync::{mpsc, oneshot};
use liberte_shared::{crypto::SymmetricKey, identity::Identity, protocol::WireMessage, types::ChannelId};
use crate::swarm::{NetworkSnapshot, SwarmCommand, SwarmNotification};

async fn result_with_timeout(receiver: oneshot::Receiver<Result<(), String>>) -> Result<(), String> {
    tokio::time::timeout(std::time::Duration::from_secs(15), receiver)
        .await.map_err(|_| "Network operation timed out".to_string())?
        .map_err(|_| "Network task stopped".to_string())?
}

/// Confirms acceptance by GossipSub, not delivery/read by a remote recipient.
pub async fn publish_confirmed(cmd_tx: &mpsc::Sender<SwarmCommand>, topic: String, data: Vec<u8>) -> Result<(), String> {
    let (reply, receiver) = oneshot::channel();
    cmd_tx.send(SwarmCommand::PublishConfirmed { topic, data, reply }).await.map_err(|_| "Network task stopped".to_string())?;
    result_with_timeout(receiver).await
}

pub async fn dial_confirmed(cmd_tx: &mpsc::Sender<SwarmCommand>, address: libp2p::Multiaddr) -> Result<(), String> {
    let (reply, receiver) = oneshot::channel();
    cmd_tx.send(SwarmCommand::DialConfirmed { address, reply }).await.map_err(|_| "Network task stopped".to_string())?;
    result_with_timeout(receiver).await
}

pub async fn network_snapshot(cmd_tx: &mpsc::Sender<SwarmCommand>) -> Result<NetworkSnapshot, String> {
    let (reply, receiver) = oneshot::channel();
    cmd_tx.send(SwarmCommand::GetSnapshot(reply)).await.map_err(|_| "Network task stopped".to_string())?;
    tokio::time::timeout(std::time::Duration::from_secs(3), receiver).await
        .map_err(|_| "Network status timed out".to_string())?.map_err(|_| "Network task stopped".to_string())
}

pub async fn subscribe_topic(cmd_tx: &mpsc::Sender<SwarmCommand>, channel_id: &ChannelId) -> anyhow::Result<()> {
    let (reply, receiver) = oneshot::channel();
    cmd_tx.send(SwarmCommand::SubscribeConfirmed { topic: channel_id.to_topic(), reply }).await
        .map_err(|_| anyhow::anyhow!("Network task stopped"))?;
    result_with_timeout(receiver).await.map_err(anyhow::Error::msg)
}

pub async fn publish_message(cmd_tx: &mpsc::Sender<SwarmCommand>, channel_id: &ChannelId, channel_key: &SymmetricKey, identity: &Identity, message: &WireMessage) -> anyhow::Result<()> {
    let encrypted = message.seal(channel_id.clone(), channel_key, identity).map_err(anyhow::Error::msg)?;
    publish_confirmed(cmd_tx, channel_id.to_topic(), encrypted).await.map_err(anyhow::Error::msg)
}

pub fn decode_message(data: &[u8], channel_id: &ChannelId, channel_key: &SymmetricKey) -> anyhow::Result<WireMessage> {
    WireMessage::open(data, channel_id.clone(), channel_key).map(|(_, message)| message).map_err(anyhow::Error::msg)
}

pub fn try_decode_notification(notification: &SwarmNotification, channel_id: &ChannelId, channel_key: &SymmetricKey) -> Option<WireMessage> {
    match notification {
        SwarmNotification::MessageReceived { topic, data, .. } if topic == &channel_id.to_topic() => decode_message(data, channel_id, channel_key).ok(),
        _ => None,
    }
}
