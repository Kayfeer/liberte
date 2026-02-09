//! DNS-over-HTTPS resolver that bypasses the OS/ISP DNS entirely.
//!
//! Forces all DNS queries through Cloudflare (1.1.1.1) and Google (8.8.8.8)
//! DoH endpoints so that local network surveillance or censorship cannot
//! interfere with peer discovery.

use hickory_resolver::{
    config::{NameServerConfig, Protocol, ResolverConfig, ResolverOpts},
    TokioAsyncResolver,
};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tracing::info;

use liberte_shared::constants::{DOH_CLOUDFLARE, DOH_GOOGLE};

/// Build a DNS-over-HTTPS async resolver that queries only Cloudflare and Google.
///
/// This resolver completely bypasses the operating system's DNS configuration,
/// preventing ISP-level DNS poisoning, logging, or censorship from affecting
/// Liberte's peer discovery and connectivity.
///
/// # Returns
///
/// A `TokioAsyncResolver` configured for DoH with both Cloudflare (1.1.1.1)
/// and Google (8.8.8.8) as upstream resolvers.
pub fn build_doh_resolver() -> TokioAsyncResolver {
    let cloudflare_addr: IpAddr = DOH_CLOUDFLARE
        .parse()
        .unwrap_or(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)));
    let google_addr: IpAddr = DOH_GOOGLE
        .parse()
        .unwrap_or(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)));

    let cloudflare_ns = NameServerConfig {
        socket_addr: SocketAddr::new(cloudflare_addr, 443),
        protocol: Protocol::Https,
        tls_dns_name: Some("cloudflare-dns.com".to_string()),
        trust_negative_responses: false,
        tls_config: None,
        bind_addr: None,
    };

    let google_ns = NameServerConfig {
        socket_addr: SocketAddr::new(google_addr, 443),
        protocol: Protocol::Https,
        tls_dns_name: Some("dns.google".to_string()),
        trust_negative_responses: false,
        tls_config: None,
        bind_addr: None,
    };

    let mut resolver_config = ResolverConfig::new();
    resolver_config.add_name_server(cloudflare_ns);
    resolver_config.add_name_server(google_ns);

    let mut opts = ResolverOpts::default();
    // Use all configured servers, not just the first
    opts.num_concurrent_reqs = 2;
    // Cache results for performance
    opts.cache_size = 256;
    // Rotate between nameservers
    opts.rotate = true;

    info!("Built DoH resolver with Cloudflare (1.1.1.1) and Google (8.8.8.8)");

    TokioAsyncResolver::tokio(resolver_config, opts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_doh_resolver_does_not_panic() {
        // Simply verify that construction succeeds without panicking.
        let _resolver = build_doh_resolver();
    }
}
