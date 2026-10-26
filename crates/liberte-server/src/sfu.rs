//! LiveKit admission only. Media and E2EE keys never pass through this API.
//! Grant schema: https://docs.livekit.io/frontends/reference/tokens-grants/
use axum::{
    extract::State,
    http::header,
    response::{IntoResponse, Response},
    Extension, Json,
};
use chrono::Utc;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde::{Deserialize, Serialize};

use crate::{api::AppState, auth::AuthenticatedUser, config::LiveKitConfig, error::ServerError};

const TOKEN_TTL_SECONDS: i64 = 300;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TokenRequest {
    pub room_name: String,
}

#[derive(Serialize, Deserialize)]
pub struct TokenResponse {
    pub url: String,
    pub token: String,
    pub room_name: String,
}

#[derive(Serialize)]
struct Claims {
    iss: String,
    sub: String,
    nbf: i64,
    exp: i64,
    video: VideoGrant,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VideoGrant {
    room: String,
    room_join: bool,
    room_create: bool,
    room_list: bool,
    room_admin: bool,
    room_record: bool,
    ingress_admin: bool,
    can_publish: bool,
    can_subscribe: bool,
    can_publish_data: bool,
    can_update_own_metadata: bool,
    can_publish_sources: Vec<&'static str>,
}

fn create_token(
    config: &LiveKitConfig,
    owner: &[u8; 32],
    room: &str,
) -> Result<TokenResponse, ServerError> {
    if !matches!(room.len(), 32 | 64) || !room.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ServerError::BadRequest(
            "Room must be a 32 or 64 character opaque hex identifier".into(),
        ));
    }
    let room = room.to_ascii_lowercase();
    let now = Utc::now().timestamp();
    let claims = Claims {
        iss: config.api_key.clone(),
        sub: hex::encode(owner),
        nbf: now,
        exp: now + TOKEN_TTL_SECONDS,
        video: VideoGrant {
            room: room.clone(),
            room_join: true,
            room_create: false,
            room_list: false,
            room_admin: false,
            room_record: false,
            ingress_admin: false,
            can_publish: true,
            can_subscribe: true,
            can_publish_data: false,
            can_update_own_metadata: false,
            can_publish_sources: vec!["camera", "microphone", "screen_share", "screen_share_audio"],
        },
    };
    let token = encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(config.api_secret.as_bytes()),
    )
    .map_err(|_| ServerError::Internal("Unable to create media token".into()))?;
    Ok(TokenResponse {
        url: config.url.clone(),
        token,
        room_name: room,
    })
}

pub async fn issue_token(
    State(state): State<AppState>,
    Extension(AuthenticatedUser(owner)): Extension<AuthenticatedUser>,
    Json(request): Json<TokenRequest>,
) -> Result<Response, ServerError> {
    let config =
        state.config.livekit.as_ref().ok_or_else(|| {
            ServerError::ServiceUnavailable("Media server is not configured".into())
        })?;
    let token = create_token(config, &owner, &request.room_name)?;
    Ok(([(header::CACHE_CONTROL, "no-store")], Json(token)).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{decode, decode_header, DecodingKey, Validation};

    fn config() -> LiveKitConfig {
        LiveKitConfig {
            url: "wss://media.example.test".into(),
            api_key: "testkey".into(),
            api_secret: "a-test-secret-longer-than-32-characters".into(),
        }
    }

    #[test]
    fn signed_grant_is_scoped_to_identity_room_and_short_ttl() {
        let config = config();
        let room = "ab".repeat(32);
        let owner = [42; 32];
        let response = create_token(&config, &owner, &room).unwrap();
        let header = decode_header(&response.token).unwrap();
        assert_eq!(header.alg, Algorithm::HS256);
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_issuer(&[&config.api_key]);
        let claims = decode::<serde_json::Value>(
            &response.token,
            &DecodingKey::from_secret(config.api_secret.as_bytes()),
            &validation,
        )
        .unwrap()
        .claims;
        assert_eq!(claims["sub"], hex::encode(owner));
        assert_eq!(claims["video"]["room"], room);
        assert_eq!(claims["video"]["roomJoin"], true);
        assert_eq!(claims["video"]["roomAdmin"], false);
        assert_eq!(claims["video"]["canPublishData"], false);
        assert_eq!(
            claims["exp"].as_i64().unwrap() - claims["nbf"].as_i64().unwrap(),
            TOKEN_TTL_SECONDS
        );
        assert!(decode::<serde_json::Value>(
            &response.token,
            &DecodingKey::from_secret(b"wrong-secret"),
            &validation
        )
        .is_err());
    }

    #[test]
    fn non_opaque_and_unbounded_room_names_are_rejected() {
        for room in [
            "",
            "public-room",
            "../room",
            &"a".repeat(65),
            &"g".repeat(64),
        ] {
            assert!(create_token(&config(), &[42; 32], room).is_err());
        }
    }
}
