use std::net::IpAddr;
use dns_lookup::lookup_addr;

pub fn resolve_addr_to_hostname(addr: IpAddr) -> Option<String> {
    match addr {
        IpAddr::V4(ipv4_addr) => {
            if ipv4_addr.is_link_local() || ipv4_addr.is_loopback() {
                return None
            }
        }
        IpAddr::V6(ipv6_addr) => {
            if ipv6_addr.is_unicast_link_local() || ipv6_addr.is_loopback() {
                return None
            }
        }
    }
    lookup_addr(&addr).ok()
} 