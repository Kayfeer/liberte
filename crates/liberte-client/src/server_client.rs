use std::time::Duration;

use liberte_shared::{http_auth::SignedRequest, identity::Identity, premium::PremiumToken};

/// Remote endpoints require TLS. Plain HTTP is allowed only for local development.
pub fn endpoint(base: &str, path: &str) -> Result<reqwest::Url, String> {
    let mut url = reqwest::Url::parse(base.trim()).map_err(|_| "URL du serveur invalide")?;
    if !url.username().is_empty() || url.password().is_some() || url.query().is_some() || url.fragment().is_some() {
        return Err("L'URL du serveur ne doit pas contenir d'identifiants, de requête ou de fragment".into());
    }
    let local = url.host_str().is_some_and(|host| {
        host.eq_ignore_ascii_case("localhost")
            || host.trim_matches(['[', ']']).parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback())
    });
    if url.scheme() != "https" && !(url.scheme() == "http" && local) {
        return Err("HTTPS est requis pour un serveur distant".into());
    }
    // API paths are absolute so signing cannot disagree with a base-path prefix.
    if url.path() != "/" && !url.path().is_empty() {
        return Err("Utilisez l'origine du serveur sans sous-chemin".into());
    }
    if !path.starts_with('/') || path.contains('?') || path.contains('#') {
        return Err("Chemin API invalide".into());
    }
    url.set_path(path);
    Ok(url)
}

pub async fn signed_request(
    base: &str,
    method: reqwest::Method,
    path: &str,
    body: Vec<u8>,
    identity: &Identity,
    premium: Option<&PremiumToken>,
    content_type: &str,
) -> Result<reqwest::Response, String> {
    let url = endpoint(base, path)?;
    let auth = SignedRequest::new(identity, method.as_str(), url.path(), &body)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(90))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| e.to_string())?;
    let mut request = client.request(method, url).header("content-type", content_type);
    for (name, value) in auth.headers() {
        request = request.header(name, value);
    }
    if let Some(token) = premium {
        request = request.header("x-liberte-premium", serde_json::to_string(token).map_err(|e| e.to_string())?);
    }
    let response = request.body(body).send().await.map_err(|e| format!("Serveur inaccessible : {e}"))?;
    if !response.status().is_success() {
        return Err(format!("Le serveur a refusé l'opération ({})", response.status()));
    }
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_plaintext_and_credential_urls_are_rejected() {
        assert!(endpoint("http://example.com", "/info").is_err());
        assert!(endpoint("https://user:password@example.com", "/info").is_err());
        assert!(endpoint("https://example.com/prefix", "/info").is_err());
        assert_eq!(endpoint("https://example.com", "/info").unwrap().as_str(), "https://example.com/info");
        assert!(endpoint("http://127.0.0.1:8080", "/info").is_ok());
    }
}
