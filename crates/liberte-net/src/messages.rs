use crate::swarm::{NetworkSnapshot, SwarmCommand, SwarmNotification};
use liberte_shared::{
    crypto::SymmetricKey, identity::Identity, protocol::WireMessage, types::ChannelId,
};
use tokio::sync::{mpsc, oneshot};

async fn request_with_timeout<T>(
    cmd_tx: &mpsc::Sender<SwarmCommand>,
    command: SwarmCommand,
    receiver: oneshot::Receiver<T>,
    duration: std::time::Duration,
) -> Result<T, String> {
    // Include the bounded command queue in the deadline: the swarm may itself be
    // waiting for the notification consumer that is requesting this operation.
    tokio::time::timeout(duration, async {
        cmd_tx
            .send(command)
            .await
            .map_err(|_| "Network task stopped".to_string())?;
        receiver
            .await
            .map_err(|_| "Network task stopped".to_string())
    })
    .await
    .map_err(|_| "Network operation timed out".to_string())?
}

/// Confirms acceptance by GossipSub, not delivery/read by a remote recipient.
pub async fn publish_confirmed(
    cmd_tx: &mpsc::Sender<SwarmCommand>,
    topic: String,
    data: Vec<u8>,
) -> Result<(), String> {
    let (reply, receiver) = oneshot::channel();
    request_with_timeout(
        cmd_tx,
        SwarmCommand::PublishConfirmed { topic, data, reply },
        receiver,
        std::time::Duration::from_secs(15),
    )
    .await?
}

pub async fn dial_confirmed(
    cmd_tx: &mpsc::Sender<SwarmCommand>,
    address: libp2p::Multiaddr,
) -> Result<(), String> {
    let (reply, receiver) = oneshot::channel();
    request_with_timeout(
        cmd_tx,
        SwarmCommand::DialConfirmed { address, reply },
        receiver,
        std::time::Duration::from_secs(15),
    )
    .await?
}

pub async fn network_snapshot(
    cmd_tx: &mpsc::Sender<SwarmCommand>,
) -> Result<NetworkSnapshot, String> {
    let (reply, receiver) = oneshot::channel();
    request_with_timeout(
        cmd_tx,
        SwarmCommand::GetSnapshot(reply),
        receiver,
        std::time::Duration::from_secs(3),
    )
    .await
}

pub async fn subscribe_topic(
    cmd_tx: &mpsc::Sender<SwarmCommand>,
    channel_id: &ChannelId,
) -> anyhow::Result<()> {
    let (reply, receiver) = oneshot::channel();
    request_with_timeout(
        cmd_tx,
        SwarmCommand::SubscribeConfirmed {
            topic: channel_id.to_topic(),
            reply,
        },
        receiver,
        std::time::Duration::from_secs(15),
    )
    .await
    .map_err(anyhow::Error::msg)?
    .map_err(anyhow::Error::msg)
}

pub async fn publish_message(
    cmd_tx: &mpsc::Sender<SwarmCommand>,
    channel_id: &ChannelId,
    channel_key: &SymmetricKey,
    identity: &Identity,
    message: &WireMessage,
) -> anyhow::Result<()> {
    let encrypted = message
        .seal(channel_id.clone(), channel_key, identity)
        .map_err(anyhow::Error::msg)?;
    publish_confirmed(cmd_tx, channel_id.to_topic(), encrypted)
        .await
        .map_err(anyhow::Error::msg)
}

pub fn decode_message(
    data: &[u8],
    channel_id: &ChannelId,
    channel_key: &SymmetricKey,
) -> anyhow::Result<WireMessage> {
    WireMessage::open(data, channel_id.clone(), channel_key)
        .map(|(_, message)| message)
        .map_err(anyhow::Error::msg)
}

pub fn try_decode_notification(
    notification: &SwarmNotification,
    channel_id: &ChannelId,
    channel_key: &SymmetricKey,
) -> Option<WireMessage> {
    match notification {
        SwarmNotification::MessageReceived { topic, data, .. }
            if topic == &channel_id.to_topic() =>
        {
            decode_message(data, channel_id, channel_key).ok()
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn full_command_queue_is_included_in_request_deadline() {
        let (tx, mut rx) = mpsc::channel(1);
        tx.send(SwarmCommand::Shutdown).await.unwrap();
        let (reply, receiver) = oneshot::channel();
        let result = request_with_timeout(
            &tx,
            SwarmCommand::GetSnapshot(reply),
            receiver,
            std::time::Duration::from_millis(20),
        )
        .await;
        assert_eq!(result.unwrap_err(), "Network operation timed out");
        assert!(matches!(rx.try_recv(), Ok(SwarmCommand::Shutdown)));
        assert!(rx.try_recv().is_err());
    }
}
