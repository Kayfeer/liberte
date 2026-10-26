use crate::state::AppState;
use liberte_shared::identity::Identity;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::State;
use uuid::Uuid;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SfuConnection {
    pub url: String,
    pub token: String,
    pub encryption_key: String,
    pub room_name: String,
}

#[derive(Deserialize)]
struct ServerConnection {
    url: String,
    token: String,
    room_name: String,
}

fn room_material(channel: Uuid, key: &[u8; 32]) -> (String, String) {
    let mut material = Vec::with_capacity(48);
    material.extend_from_slice(channel.as_bytes());
    material.extend_from_slice(key);
    let room = blake3::derive_key("liberte-sfu-room-name-v1", &material);
    let encryption = blake3::derive_key("liberte-sfu-media-e2ee-v1", &material);
    (hex::encode(room), hex::encode(encryption))
}

// This isolated prototype links CEF on Linux. The production WebKit branch
// keeps its platform refusal. SfuSession still requires encoded transforms,
// enables E2EE before capture/publication and rejects unencrypted remote tracks.
#[tauri::command]
pub async fn get_sfu_connection(
    state: State<'_, Arc<Mutex<AppState>>>,
    channel_id: String,
) -> Result<SfuConnection, String> {
    let channel = Uuid::parse_str(&channel_id).map_err(|e| e.to_string())?;
    let (server, identity, token, key): (String, Identity, _, [u8; 32]) = {
        let mut guard = state.lock().map_err(|e| e.to_string())?;
        crate::commands::premium::restore_status(&mut guard);
        let db = guard.database.as_ref().ok_or("Base indisponible")?;
        let key: [u8; 32] = hex::decode(db.get_channel_key(channel).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?
            .try_into()
            .map_err(|_| "Clé de canal invalide")?;
        (
            guard.server_url.clone(),
            guard.identity.clone().ok_or("Aucune identité chargée")?,
            guard.premium_token.clone(),
            key,
        )
    };
    let (room_name, encryption_key) = room_material(channel, &key);
    // Only an opaque keyed room identifier leaves this device. The media key is
    // returned through local IPC and never included in the signed HTTP request.
    let body = serde_json::to_vec(&serde_json::json!({"room_name":room_name}))
        .map_err(|e| e.to_string())?;
    let response = crate::server_client::signed_request(
        &server,
        reqwest::Method::POST,
        "/media/token",
        body,
        &identity,
        token.as_ref(),
        "application/json",
    )
    .await?;
    let connection: ServerConnection =
        crate::server_client::read_json_limited(response, 16384).await?;
    if connection.room_name != room_name || connection.token.len() > 8192 {
        return Err("Réponse SFU invalide".into());
    }
    let url = reqwest::Url::parse(&connection.url).map_err(|_| "URL SFU invalide")?;
    let local = url.host_str().is_some_and(|host| {
        host.eq_ignore_ascii_case("localhost")
            || host
                .trim_matches(['[', ']'])
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
    });
    if !url.username().is_empty()
        || url.password().is_some()
        || (url.scheme() != "wss" && !(local && url.scheme() == "ws"))
    {
        return Err("Le SFU distant doit utiliser WSS".into());
    }
    Ok(SfuConnection {
        url: connection.url,
        token: connection.token,
        room_name,
        encryption_key,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn room_and_media_keys_are_separate_and_depend_on_channel_secret() {
        let channel = Uuid::new_v4();
        let a = room_material(channel, &[1; 32]);
        assert_eq!(a, room_material(channel, &[1; 32]));
        assert_ne!(a.0, a.1);
        assert_ne!(a, room_material(channel, &[2; 32]));
        assert_ne!(a, room_material(Uuid::new_v4(), &[1; 32]));
    }
}
