use hickory_resolver::{
    config::{NameServerConfig, ResolveHosts, ResolverConfig, ServerOrderingStrategy},
    net::runtime::TokioRuntimeProvider,
    Resolver, TokioResolver,
};
use std::net::{IpAddr, Ipv4Addr};
use tracing::info;

use liberte_shared::constants::{DOH_CLOUDFLARE, DOH_GOOGLE};

fn doh_config() -> ResolverConfig {
    let cloudflare_addr: IpAddr = DOH_CLOUDFLARE
        .parse()
        .unwrap_or(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)));
    let google_addr: IpAddr = DOH_GOOGLE
        .parse()
        .unwrap_or(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)));
    let mut config = ResolverConfig::from_name_servers(Vec::new());
    for (ip, name) in [
        (cloudflare_addr, "cloudflare-dns.com"),
        (google_addr, "dns.google"),
    ] {
        let mut server = NameServerConfig::https(ip, name.into(), None);
        server.trust_negative_responses = false;
        config.add_name_server(server);
    }
    config
}

/// Builds a DoH resolver that bypasses OS/ISP DNS entirely.
pub fn build_doh_resolver() -> anyhow::Result<TokioResolver> {
    let mut builder = Resolver::builder_with_config(doh_config(), TokioRuntimeProvider::default());
    let options = builder.options_mut();
    options.num_concurrent_reqs = 2;
    options.cache_size = 256;
    options.server_ordering_strategy = ServerOrderingStrategy::RoundRobin;
    options.use_hosts_file = ResolveHosts::Never;

    info!("Built DoH resolver with Cloudflare (1.1.1.1) and Google (8.8.8.8)");
    builder.build().map_err(anyhow::Error::from)
}

/// Resolve explicit DNS components through DoH; never fall back to the system resolver.
pub async fn resolve_multiaddr(address: libp2p::Multiaddr) -> anyhow::Result<libp2p::Multiaddr> {
    use libp2p::multiaddr::Protocol;
    let mut resolved = libp2p::Multiaddr::empty();
    for component in address.iter() {
        match component {
            Protocol::Dns(ref name) | Protocol::Dns4(ref name) | Protocol::Dns6(ref name) => {
                let lookup = build_doh_resolver()?.lookup_ip(name.as_ref()).await?;
                let ip = lookup
                    .iter()
                    .find(|ip| match &component {
                        Protocol::Dns4(_) => ip.is_ipv4(),
                        Protocol::Dns6(_) => ip.is_ipv6(),
                        _ => true,
                    })
                    .ok_or_else(|| anyhow::anyhow!("No matching IP address"))?;
                resolved.push(match ip {
                    IpAddr::V4(ip) => Protocol::Ip4(ip),
                    IpAddr::V6(ip) => Protocol::Ip6(ip),
                });
            }
            Protocol::Dnsaddr(_) => {
                return Err(anyhow::anyhow!(
                    "DNSADDR is not supported; use /dns, /dns4 or /dns6"
                ))
            }
            other => resolved.push(other),
        }
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hickory_resolver::config::ProtocolConfig;

    #[test]
    fn doh_configuration_uses_only_authenticated_https_endpoints() {
        let config = doh_config();
        assert_eq!(config.name_servers().len(), 2);
        for server in config.name_servers() {
            assert!(!server.trust_negative_responses);
            assert_eq!(server.connections.len(), 1);
            let connection = &server.connections[0];
            assert_eq!(connection.port, 443);
            let ProtocolConfig::Https { server_name, path } = &connection.protocol else {
                panic!("DNS must not fall back to plaintext");
            };
            assert!(matches!(
                server_name.as_ref(),
                "cloudflare-dns.com" | "dns.google"
            ));
            assert_eq!(path.as_ref(), "/dns-query");
        }
        let resolver = build_doh_resolver().unwrap();
        assert_eq!(resolver.options().use_hosts_file, ResolveHosts::Never);
    }
}
