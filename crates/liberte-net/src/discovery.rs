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
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| {
            match line.parse::<Multiaddr>() {
                Ok(addr) => {
                    debug!(addr = %addr, "Loaded bootstrap peer");
                    Some(addr)
                }
                Err(e) => {
                    warn!(line = %line, error = %e, "Skipping invalid multiaddr");
                    None
                }
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
        writeln!(f, "").unwrap();
        writeln!(f, "invalid-addr").unwrap();
        writeln!(f, "/ip4/127.0.0.2/udp/4001/quic-v1").unwrap();
        drop(f);

        let peers = load_bootstrap_peers(&file_path);
        assert_eq!(peers.len(), 2);

        let _ = fs::remove_file(&file_path);
        let _ = fs::remove_dir(&dir);
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
