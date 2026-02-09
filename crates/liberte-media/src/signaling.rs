use liberte_shared::protocol::{SignalMessage, SignalType, WireMessage};
use liberte_shared::types::{ChannelId, UserId};
use tracing::debug;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignalingState {
    Idle,
    OfferSent,
    OfferReceived,
    Connected,
    Closed,
}

pub struct SignalingSession {
    pub local_user: UserId,
    pub remote_user: UserId,
    pub channel_id: ChannelId,
    pub state: SignalingState,
    pub local_sdp: Option<String>,
    pub remote_sdp: Option<String>,
    pub ice_candidates: Vec<String>,
}

impl SignalingSession {
    pub fn new(local_user: UserId, remote_user: UserId, channel_id: ChannelId) -> Self {
        Self {
            local_user,
            remote_user,
            channel_id,
            state: SignalingState::Idle,
            local_sdp: None,
            remote_sdp: None,
            ice_candidates: Vec::new(),
        }
    }

    pub fn create_offer(&mut self, sdp: String) -> WireMessage {
        self.local_sdp = Some(sdp.clone());
        self.state = SignalingState::OfferSent;
        debug!(
            remote = %self.remote_user.short(),
            "Creating SDP offer"
        );

        WireMessage::Signal(SignalMessage {
            sender: self.local_user.clone(),
            target: self.remote_user.clone(),
            channel_id: self.channel_id.clone(),
            signal_type: SignalType::Offer(sdp),
        })
    }

    pub fn create_answer(&mut self, sdp: String) -> WireMessage {
        self.local_sdp = Some(sdp.clone());
        self.state = SignalingState::Connected;
        debug!(
            remote = %self.remote_user.short(),
            "Creating SDP answer"
        );

        WireMessage::Signal(SignalMessage {
            sender: self.local_user.clone(),
            target: self.remote_user.clone(),
            channel_id: self.channel_id.clone(),
            signal_type: SignalType::Answer(sdp),
        })
    }

    pub fn create_ice_candidate(&mut self, candidate: String) -> WireMessage {
        self.ice_candidates.push(candidate.clone());

        WireMessage::Signal(SignalMessage {
            sender: self.local_user.clone(),
            target: self.remote_user.clone(),
            channel_id: self.channel_id.clone(),
            signal_type: SignalType::IceCandidate(candidate),
        })
    }

    pub fn handle_signal(&mut self, signal: &SignalMessage) -> SignalingAction {
        match &signal.signal_type {
            SignalType::Offer(sdp) => {
                self.remote_sdp = Some(sdp.clone());
                self.state = SignalingState::OfferReceived;
                debug!(from = %signal.sender.short(), "Received SDP offer");
                SignalingAction::CreateAnswer
            }
            SignalType::Answer(sdp) => {
                self.remote_sdp = Some(sdp.clone());
                self.state = SignalingState::Connected;
                debug!(from = %signal.sender.short(), "Received SDP answer");
                SignalingAction::SetRemoteDescription
            }
            SignalType::IceCandidate(candidate) => {
                debug!(from = %signal.sender.short(), "Received ICE candidate");
                SignalingAction::AddIceCandidate(candidate.clone())
            }
            SignalType::Hangup => {
                self.state = SignalingState::Closed;
                debug!(from = %signal.sender.short(), "Received hangup");
                SignalingAction::Close
            }
        }
    }

    pub fn hangup(&mut self) -> WireMessage {
        self.state = SignalingState::Closed;

        WireMessage::Signal(SignalMessage {
            sender: self.local_user.clone(),
            target: self.remote_user.clone(),
            channel_id: self.channel_id.clone(),
            signal_type: SignalType::Hangup,
        })
    }
}

#[derive(Debug)]
pub enum SignalingAction {
    CreateAnswer,
    SetRemoteDescription,
    AddIceCandidate(String),
    Close,
}
