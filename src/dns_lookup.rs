use hickory_resolver::TokioAsyncResolver;
use hickory_resolver::config::{NameServerConfigGroup, ResolverConfig, ResolverOpts};
use std::net::IpAddr;

// Risolve l'hostname usando il server DNS indicato (porta 53, UDP con fallback TCP).
// Se dns_server_ip_str è vuoto usa la configurazione DNS di sistema.
pub async fn resolve_hostname_with_specific_dns(
    hostname: &str,
    dns_server_ip_str: &str,
) -> Result<String, String> {
    let dns_server_ip_str = dns_server_ip_str.trim();

    let resolver = if dns_server_ip_str.is_empty() {
        TokioAsyncResolver::tokio_from_system_conf().unwrap_or_else(|_| {
            TokioAsyncResolver::tokio(ResolverConfig::default(), ResolverOpts::default())
        })
    } else {
        let dns_ip: IpAddr = dns_server_ip_str
            .parse()
            .map_err(|e| format!("IP del server DNS non valido '{}': {}", dns_server_ip_str, e))?;

        let name_servers = NameServerConfigGroup::from_ips_clear(&[dns_ip], 53, true);
        let config = ResolverConfig::from_parts(None, vec![], name_servers);
        TokioAsyncResolver::tokio(config, ResolverOpts::default())
    };

    let response = resolver
        .lookup_ip(hostname)
        .await
        .map_err(|e| format!("Impossibile risolvere '{}': {}", hostname, e))?;

    response
        .iter()
        .next()
        .map(|ip| ip.to_string())
        .ok_or_else(|| format!("Nessun indirizzo IP trovato per '{}'", hostname))
}
