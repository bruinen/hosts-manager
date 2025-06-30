use trust_dns_resolver::Resolver;
use trust_dns_resolver::config::{ResolverConfig, ResolverOpts, NameServerConfig};
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr; 


pub fn resolve_hostname_with_specific_dns(
    hostname: &str,
    dns_server_ip_str: &str,
) -> io::Result<String> {
    let mut resolver = Resolver::new(ResolverConfig::default(), ResolverOpts::default())?;
    if !dns_server_ip_str.is_empty() {       
        let dns_ip = IpAddr::from_str(dns_server_ip_str)
            .map_err(|e| io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Invalid DNS server IP '{}': {}", dns_server_ip_str, e)
            ))?;
    
        // La porta standard per il DNS è la 53 (UDP)
        let dns_socket_addr = SocketAddr::new(dns_ip, 53);
    
        let mut config = ResolverConfig::new();
        config.add_name_server(NameServerConfig {
            socket_addr: dns_socket_addr,
            protocol: trust_dns_resolver::config::Protocol::Udp, // Il protocollo UDP è il più comune per DNS
            tls_dns_name: None, // Usa None per DNS su UDP/TCP standard
            trust_negative_responses: false,
            bind_addr: None,
        });
    
        resolver = Resolver::new(config, ResolverOpts::default())?;
       
    }
    let response = resolver.lookup_ip(hostname)
        .map_err(|e| io::Error::new(
            io::ErrorKind::Other,
            format!("Failed to resolve hostname '{}': {}", hostname, e)))?;
    
    let first_ip = response.iter()
        .next()
        .ok_or_else(|| io::Error::new(
            io::ErrorKind::NotFound,
            "No IP addresses found for hostname"
        ))?;

    Ok(first_ip.to_string())
}
