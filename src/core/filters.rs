use super::connection::Connection;


#[derive(Debug, Clone, Default)]
pub struct ConnectionFilter {
    pub pid: Option<u32>,
    pub process_name: Option<String>,
    pub remote_host: Option<String>,
    pub remote_port: Option<u16>,
}

impl ConnectionFilter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_pid(mut self, pid: u32) -> Self {
        self.pid = Some(pid);
        self
    }

    pub fn with_process_name(mut self, name: String) -> Self {
        self.process_name = Some(name);
        self
    }

    pub fn with_remote_host(mut self, host: String) -> Self {
        self.remote_host = Some(host);
        self
    }

    pub fn with_remote_port(mut self, port: u16) -> Self {
        self.remote_port = Some(port);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.pid.is_none() && 
        self.process_name.is_none() && 
        self.remote_host.is_none() && 
        self.remote_port.is_none()
    }

    pub fn to_string(&self) -> String {
        let mut parts = Vec::new();
        
        if let Some(pid) = self.pid {
            parts.push(format!("PID: {}", pid));
        }
        
        if let Some(ref process_name) = self.process_name {
            parts.push(format!("Process: {}", process_name));
        }
        
        if let Some(ref remote_host) = self.remote_host {
            parts.push(format!("Host: {}", remote_host));
        }
        
        if let Some(port) = self.remote_port {
            parts.push(format!("Port: {}", port));
        }
        
        if parts.is_empty() {
            "No filters".to_string()
        } else {
            parts.join(", ")
        }
    }

    pub fn matches_connection(&self, conn: &Connection, process_name: Option<&str>) -> bool {
        // If any filter doesn't match, return false
        if let Some(pid) = self.pid {
            if conn.pid != pid {
                return false;
            }
        }

        if let Some(ref process_filter) = self.process_name {
            if let Some(name) = process_name {
                if !name.contains(process_filter) {
                    return false;
                }
            } else {
                return false;
            }
        }

        if let Some(ref host_filter) = self.remote_host {
            if let Some(ref hostname) = conn.remote_hostname {
                if !hostname.contains(host_filter) {
                    let addr_str = conn.remote_addr.to_string();
                    if !addr_str.contains(host_filter) {
                        return false;
                    }
                }
            } else {
                // No hostname, check IP address directly
                let addr_str = conn.remote_addr.to_string();
                if !addr_str.contains(host_filter) {
                    return false;
                }
            }
        }

        if let Some(port) = self.remote_port {
            if conn.remote_port != port {
                return false;
            }
        }

        // If we got here, all specified filters matched
        true
    }
} 