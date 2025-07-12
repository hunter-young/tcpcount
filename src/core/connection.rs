use std::net::IpAddr;
use netstat2::TcpState;
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct Connection {
    pub id: u64,                       // Unique connection identifier
    pub pid: u32,                      // Process ID
    pub local_port: u16,               // Local port
    pub remote_port: u16,              // Remote port
    pub remote_addr: IpAddr,           // Remote IP address
    pub remote_hostname: Option<String>, // Resolved hostname
    pub state: TcpState,               // TCP state
    pub first_seen: SystemTime,        // When connection was first observed
    pub last_seen: SystemTime,         // When connection was last observed
    pub closed: bool,                  // Whether connection is closed
}

impl Connection {
    pub fn new(
        pid: u32,
        local_port: u16,
        remote_port: u16,
        remote_addr: IpAddr,
        remote_hostname: Option<String>,
        state: TcpState,
    ) -> Self {
        let now = SystemTime::now();
        Self {
            id: rand::random(),
            pid,
            local_port,
            remote_port,
            remote_addr,
            remote_hostname,
            state,
            first_seen: now,
            last_seen: now,
            closed: false,
        }
    }

    pub fn update_state(&mut self, state: TcpState) {
        self.state = state;
        self.last_seen = SystemTime::now();
    }

    pub fn mark_closed(&mut self) {
        self.closed = true;
        self.last_seen = SystemTime::now();
    }
}