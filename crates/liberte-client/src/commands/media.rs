use std::sync::{Arc, Mutex};
use serde::Serialize;
use tauri::State;
use liberte_net::SwarmCommand;
use liberte_shared::protocol::{SignalMessage, SignalType, VoiceEvent, VoiceEventType, WireMessage};
use liberte_shared::types::{ChannelId, UserId};
use crate::events::{emit_event, EVENT_CALL_STATE_CHANGED};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallState {
    pub in_call: bool,
    pub channel_id: Option<String>,
    pub is_muted: bool,
    pub is_video_enabled: bool,
    pub mode: String,
}

fn snapshot(state: &AppState) -> CallState {
    CallState {
        in_call: state.is_in_call,
        channel_id: state.call_channel_id.clone(),
        is_muted: state.is_muted,
        is_video_enabled: state.is_video_enabled,
        mode: state.call_mode.clone(),
    }
}

/// SDP is identity-signed before WebRTC sees it. Media uses DTLS-SRTP,
/// and raw audio is never transported over GossipSub.
fn seal_message(state: &AppState, channel: &ChannelId, message: &WireMessage) -> Result<Vec<u8>, String> {
    let identity = state.identity.as_ref().ok_or("No identity loaded")?;
    let db = state.database.as_ref().ok_or("No database")?;
    db.get_channel(channel.0).map_err(|_| "Unknown channel")?;
    let encoded = db.get_channel_key(channel.0).map_err(|e| e.to_string())?;
    let decoded = hex::decode(encoded).map_err(|_| "Invalid channel key")?;
    let key: [u8; 32] = decoded.try_into().map_err(|_| "Invalid channel key length")?;
    message.seal(channel.clone(), &key, identity)
}

fn announce(state: &AppState, event_type: VoiceEventType) -> Result<(), String> {
    // LiveKit supplies its own roster. Mixing mesh and SFU presence would cause
    // mesh clients to negotiate with peers using a different media transport.
    if state.call_mode == "sfu" { return Ok(()); }
    let channel = ChannelId(uuid::Uuid::parse_str(
        state.call_channel_id.as_deref().ok_or("Not in a call")?,
    ).map_err(|_| "Invalid channel UUID")?);
    let user_id = state.identity.as_ref().ok_or("No identity loaded")?.user_id();
    let message = WireMessage::VoiceEvent(VoiceEvent {
        user_id, channel_id: channel.clone(), event_type, timestamp: chrono::Utc::now(),
    });
    let data = seal_message(state, &channel, &message)?;
    // Presence is allowed in an empty room. Periodic announcements discover
    // members arriving later; SDP publication, unlike presence, is confirmed.
    state.swarm_cmd_tx.as_ref().ok_or("Network not started")?
        .try_send(SwarmCommand::PublishMessage { topic: channel.to_topic(), data })
        .map_err(|e| format!("Cannot announce call: {e}"))
}

#[tauri::command]
pub fn start_call(state: State<'_, Arc<Mutex<AppState>>>, channel_id: String) -> Result<CallState, String> {
    let channel_id = uuid::Uuid::parse_str(&channel_id).map_err(|_| "Invalid channel UUID")?.to_string();
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    if guard.is_in_call { return Err("Already in a call".into()); }
    let channel = ChannelId(uuid::Uuid::parse_str(&channel_id).map_err(|_| "Invalid channel UUID")?);
    let identity = guard.identity.as_ref().ok_or("No identity loaded")?;
    // Validate local membership and key even for SFU, which has no GossipSub presence.
    let membership_probe = WireMessage::VoiceEvent(VoiceEvent {
        user_id: identity.user_id(), channel_id: channel.clone(),
        event_type: VoiceEventType::Join, timestamp: chrono::Utc::now(),
    });
    seal_message(&guard, &channel, &membership_probe)?;
    guard.call_channel_id = Some(channel_id);
    if let Err(error) = announce(&guard, VoiceEventType::Join) {
        guard.call_channel_id = None;
        return Err(error);
    }
    guard.is_in_call = true;
    guard.is_muted = false;
    guard.is_video_enabled = false;
    let result = snapshot(&guard);
    if let Some(app) = &guard.app_handle { emit_event(app, EVENT_CALL_STATE_CHANGED, result.clone()); }
    Ok(result)
}

#[tauri::command]
pub fn announce_call(state: State<'_, Arc<Mutex<AppState>>>) -> Result<(), String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    if !guard.is_in_call { return Err("Not in a call".into()); }
    announce(&guard, VoiceEventType::Join)?;
    if guard.is_muted { announce(&guard, VoiceEventType::Mute)?; }
    Ok(())
}

#[tauri::command]
pub fn end_call(state: State<'_, Arc<Mutex<AppState>>>) -> Result<CallState, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    if guard.is_in_call {
        // Always release local state, including after a network failure.
        if let Err(error) = announce(&guard, VoiceEventType::Leave) { tracing::warn!(%error, "Call leave announcement failed"); }
    }
    guard.call_channel_id = None;
    guard.is_in_call = false;
    guard.is_muted = false;
    guard.is_video_enabled = false;
    if let Some(active) = guard.voice_active.take() {
        active.store(false, std::sync::atomic::Ordering::SeqCst);
    }
    guard.voice_playback_tx = None;
    guard.voice_muted = None;
    let result = snapshot(&guard);
    if let Some(app) = &guard.app_handle { emit_event(app, EVENT_CALL_STATE_CHANGED, result.clone()); }
    Ok(result)
}

fn validate_signal(signal: &SignalType) -> Result<(), String> {
    match signal {
        SignalType::Offer(sdp) | SignalType::Answer(sdp) if sdp.is_empty() || sdp.len() > 128 * 1024 => Err("Invalid SDP size".into()),
        SignalType::IceCandidate(candidate) if candidate.len() > 8 * 1024 => Err("Invalid ICE candidate size".into()),
        _ => Ok(()),
    }
}

#[tauri::command]
pub async fn send_call_signal(
    state: State<'_, Arc<Mutex<AppState>>>, channel_id: String, target: String, signal_type: SignalType,
) -> Result<(), String> {
    validate_signal(&signal_type)?;
    let channel = ChannelId(uuid::Uuid::parse_str(&channel_id).map_err(|_| "Invalid channel UUID")?);
    let canonical_channel = channel.0.to_string();
    let bytes = hex::decode(&target).map_err(|_| "Invalid target identity")?;
    let target = UserId(bytes.try_into().map_err(|_| "Invalid target identity length")?);
    let (tx, data) = {
        let guard = state.lock().map_err(|e| e.to_string())?;
        if !guard.is_in_call || guard.call_mode != "mesh" || guard.call_channel_id.as_deref() != Some(canonical_channel.as_str()) {
            return Err("Signal does not belong to the active call".into());
        }
        let sender = guard.identity.as_ref().ok_or("No identity loaded")?.user_id();
        if sender == target { return Err("Cannot signal yourself".into()); }
        let message = WireMessage::Signal(SignalMessage { sender, target, channel_id: channel.clone(), signal_type });
        (guard.swarm_cmd_tx.clone().ok_or("Network not started")?, seal_message(&guard, &channel, &message)?)
    };
    liberte_net::publish_confirmed(&tx, channel.to_topic(), data).await
}

#[tauri::command]
pub fn toggle_mute(state: State<'_, Arc<Mutex<AppState>>>, muted: Option<bool>) -> Result<bool, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    if !guard.is_in_call { return Err("Not in a call".into()); }
    let muted = muted.unwrap_or(!guard.is_muted);
    // Presence failure must never prevent silencing the microphone.
    let event = if muted { VoiceEventType::Mute } else { VoiceEventType::Unmute };
    if let Err(error) = announce(&guard, event) { tracing::warn!(%error, "Mute announcement failed"); }
    guard.is_muted = muted;
    Ok(muted)
}

#[tauri::command]
pub fn toggle_video(state: State<'_, Arc<Mutex<AppState>>>, enabled: Option<bool>) -> Result<bool, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    if !guard.is_in_call { return Err("Not in a call".into()); }
    guard.is_video_enabled = enabled.unwrap_or(!guard.is_video_enabled);
    Ok(guard.is_video_enabled)
}

#[tauri::command]
pub fn set_call_mode(state: State<'_, Arc<Mutex<AppState>>>, mode: String) -> Result<String, String> {
    if mode != "mesh" && mode != "sfu" { return Err("Invalid call mode".into()); }
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    if guard.is_in_call { return Err("Quittez l'appel avant de changer de transport.".into()); }
    guard.call_mode = mode.clone();
    Ok(mode)
}

#[tauri::command]
pub fn get_call_state(state: State<'_, Arc<Mutex<AppState>>>) -> Result<CallState, String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    Ok(snapshot(&guard))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signaling_payloads_are_bounded() {
        assert!(validate_signal(&SignalType::Offer(String::new())).is_err());
        assert!(validate_signal(&SignalType::Answer("x".repeat(128 * 1024 + 1))).is_err());
        assert!(validate_signal(&SignalType::IceCandidate("x".repeat(8193))).is_err());
        assert!(validate_signal(&SignalType::Offer("v=0\r\n".into())).is_ok());
        assert!(validate_signal(&SignalType::Hangup).is_ok());
    }
}
