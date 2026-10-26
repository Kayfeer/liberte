use std::fs;
use std::path::Path;

use libp2p::Multiaddr;
use tracing::{debug, info, warn};

pub fn load_bootstrap_peers(path: &Path) -> Vec<Multiaddr> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            warn!(
                path = %path.display(),
                error = %e,
                "Failed to read bootstrap peers file"
            );
            return Vec::new();
        }
    };

    let addrs: Vec<Multiaddr> = content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with('['))
        .map(|line| {
            if let Some(value) = line
                .strip_prefix("addr")
                .and_then(|v| v.trim_start().strip_prefix('='))
            {
                value.trim().trim_matches('"')
            } else {
                line
            }
        })
        .filter_map(|line| match line.parse::<Multiaddr>() {
            Ok(addr) => {
                debug!(addr = %addr, "Loaded bootstrap peer");
                Some(addr)
            }
            Err(e) => {
                warn!(line = %line, error = %e, "Skipping invalid multiaddr");
                None
            }
        })
        .collect();

    info!(
        count = addrs.len(),
        path = %path.display(),
        "Loaded bootstrap peers"
    );

    addrs
}

/// Resolve configured bootstrap names through the same authenticated DoH path as
/// manual connections. Bound both concurrency and total startup delay; a broken
/// resolver must not prevent the local identity and LAN discovery from starting.
pub async fn resolve_bootstrap_peers(addresses: Vec<Multiaddr>) -> Vec<Multiaddr> {
    use futures::StreamExt;
    use std::time::Duration;

    let pending = futures::stream::iter(addresses.into_iter().take(1024))
        .map(|address| async move {
            match tokio::time::timeout(
                Duration::from_secs(5),
                crate::dns::resolve_multiaddr(address.clone()),
            )
            .await
            {
                Ok(Ok(resolved)) => Some(resolved),
                Ok(Err(error)) => {
                    warn!(%address, %error, "Bootstrap address could not be resolved");
                    None
                }
                Err(_) => {
                    warn!(%address, "Bootstrap DNS lookup timed out");
                    None
                }
            }
        })
        .buffer_unordered(8);
    tokio::pin!(pending);
    let deadline = tokio::time::sleep(Duration::from_secs(6));
    tokio::pin!(deadline);
    let mut resolved = Vec::new();
    loop {
        tokio::select! {
            result = pending.next() => match result {
                Some(Some(address)) => resolved.push(address),
                Some(None) => {},
                None => break,
            },
            _ = &mut deadline => {
                warn!("Bootstrap resolution deadline reached; unresolved hints require another connection attempt");
                break;
            }
        }
    }
    resolved
}

/// Parse raw multiaddr strings into validated values.
pub fn parse_multiaddrs(raw: &[String]) -> Vec<Multiaddr> {
    raw.iter()
        .filter_map(|s| {
            s.parse::<Multiaddr>().ok().or_else(|| {
                warn!(addr = %s, "Could not parse multiaddr");
                None
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_load_from_nonexistent_file() {
        let peers = load_bootstrap_peers(Path::new("/nonexistent/bootstrap.txt"));
        assert!(peers.is_empty());
    }

    #[test]
    fn test_load_from_file() {
        let dir = std::env::temp_dir().join("liberte_test_bootstrap");
        let _ = fs::create_dir_all(&dir);
        let file_path = dir.join("peers.txt");

        let mut f = fs::File::create(&file_path).unwrap();
        writeln!(f, "# bootstrap nodes").unwrap();
        writeln!(f, "/ip4/127.0.0.1/udp/4001/quic-v1").unwrap();
        writeln!(f).unwrap();
        writeln!(f, "invalid-addr").unwrap();
        writeln!(f, "/ip4/127.0.0.2/udp/4001/quic-v1").unwrap();
        drop(f);

        let peers = load_bootstrap_peers(&file_path);
        assert_eq!(peers.len(), 2);

        let _ = fs::remove_file(&file_path);
        let _ = fs::remove_dir(&dir);
    }

    #[tokio::test]
    async fn bootstrap_resolution_preserves_ip_and_rejects_dnsaddr() {
        let ip: Multiaddr = "/ip4/127.0.0.1/udp/4001/quic-v1".parse().unwrap();
        let unsupported: Multiaddr = "/dnsaddr/bootstrap.invalid".parse().unwrap();
        let result = resolve_bootstrap_peers(vec![unsupported, ip.clone()]).await;
        assert_eq!(result, vec![ip]);
    }

    #[test]
    fn test_parse_multiaddrs() {
        let raw = vec![
            "/ip4/127.0.0.1/udp/4001/quic-v1".to_string(),
            "not-a-multiaddr".to_string(),
            "/ip4/10.0.0.1/udp/4001/quic-v1".to_string(),
        ];
        let addrs = parse_multiaddrs(&raw);
        assert_eq!(addrs.len(), 2);
    }
}
