use std::collections::{HashMap, HashSet};
use std::time::SystemTime;

use netstat2::{get_sockets_info, AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo, TcpState};
use sysinfo::{System, RefreshKind, Pid, ProcessStatus, ProcessRefreshKind, ProcessesToUpdate};

use super::connection::Connection;
use super::process::Process;
use super::utils::resolve_addr_to_hostname;
use super::filters::ConnectionFilter;

#[derive(Debug, Clone)]
pub struct HostMetrics {
    pub host: String,
    pub port: u16,
    pub current_connections: usize,
    pub total_connections: usize,
    pub max_concurrent: usize,
}

#[derive(Debug, Clone)]
pub struct ProcessMetrics {
    pub pid: u32,
    pub name: String,
    pub current_connections: usize,
    pub total_connections: usize,
    pub max_concurrent: usize,
    pub is_alive: bool,
}

#[derive(Debug, Clone)]
pub struct ProcessHostMetrics {
    pub pid: u32,
    pub process_name: String,
    pub host: String,
    pub port: u16,
    pub current_connections: usize,
    pub total_connections: usize,
    pub max_concurrent: usize,
    pub is_alive: bool,
}

pub struct ConnectionMetrics {
    pub total_connections_by_pid: HashMap<u32, usize>,
    pub max_concurrent_by_pid: HashMap<u32, usize>,
    pub current_concurrent_by_pid: HashMap<u32, usize>,
    pub total_connections_by_host: HashMap<String, usize>,
    pub max_concurrent_by_host: HashMap<String, usize>,
    pub current_concurrent_by_host: HashMap<String, usize>,
    pub total_connections_by_process_host: HashMap<(u32, String, u16), usize>,
    pub max_concurrent_by_process_host: HashMap<(u32, String, u16), usize>,
    pub current_concurrent_by_process_host: HashMap<(u32, String, u16), usize>,
    pub memory_history: HashMap<u32, Vec<(SystemTime, u64)>>,
    pub sample_timestamps: Vec<SystemTime>,
}

pub struct ConnectionMonitor {
    connections: HashMap<u64, Connection>,
    historical_connections: Vec<Connection>,
    processes: HashMap<u32, Process>,
    system_info: System,
    last_refresh: SystemTime,
    pub metrics: ConnectionMetrics,
}

impl ConnectionMonitor {
    pub fn new() -> Self {
        let refresh_kind = RefreshKind::nothing().with_processes(ProcessRefreshKind::everything());
        let sys = System::new_with_specifics(refresh_kind);
        
        let mut instance = Self {
            connections: HashMap::new(),
            historical_connections: Vec::new(),
            processes: HashMap::new(),
            system_info: sys,
            last_refresh: SystemTime::now(),
            metrics: ConnectionMetrics {
                total_connections_by_pid: HashMap::new(),
                max_concurrent_by_pid: HashMap::new(),
                current_concurrent_by_pid: HashMap::new(),
                total_connections_by_host: HashMap::new(),
                max_concurrent_by_host: HashMap::new(),
                current_concurrent_by_host: HashMap::new(),
                total_connections_by_process_host: HashMap::new(),
                max_concurrent_by_process_host: HashMap::new(),
                current_concurrent_by_process_host: HashMap::new(),
                memory_history: HashMap::new(),
                sample_timestamps: Vec::new(),
            },
        };
        
        instance.refresh().ok();
        instance
    }

    pub fn reset(&mut self) {
        self.connections.clear();
        self.historical_connections.clear();

        self.metrics = ConnectionMetrics {
            total_connections_by_pid: HashMap::new(),
            max_concurrent_by_pid: HashMap::new(),
            current_concurrent_by_pid: HashMap::new(),
            total_connections_by_host: HashMap::new(),
            max_concurrent_by_host: HashMap::new(),
            current_concurrent_by_host: HashMap::new(),
            total_connections_by_process_host: HashMap::new(),
            max_concurrent_by_process_host: HashMap::new(),
            current_concurrent_by_process_host: HashMap::new(),
            memory_history: HashMap::new(),
            sample_timestamps: Vec::new(),
        };
        self.processes.clear();
        self.last_refresh = SystemTime::now();
    }

    pub fn refresh(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let now = SystemTime::now();
        
        let af_flags = AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6;
        let proto_flags = ProtocolFlags::TCP;
        let sockets_info = get_sockets_info(af_flags, proto_flags)?;
        
        let current_socket_info: Vec<_> = sockets_info.into_iter()
            .filter(|si| {
                if let ProtocolSocketInfo::Tcp(tcp_si) = &si.protocol_socket_info { 
                    tcp_si.state != TcpState::Listen
                } else {
                    false
                }
            })
            .collect();
        
        let mut seen_connections = HashSet::new();
        
        self.system_info.refresh_processes(ProcessesToUpdate::All, true);
        
        // Process current connections
        for si in current_socket_info {
            if let ProtocolSocketInfo::Tcp(tcp_si) = &si.protocol_socket_info {
                if si.associated_pids.is_empty() {
                    continue;
                }
                
                let pid = si.associated_pids[0];
                let remote_hostname = resolve_addr_to_hostname(tcp_si.remote_addr);
                
                let conn_exists = self.connections.iter().find(|(_, conn)| {
                    conn.pid == pid &&
                    conn.local_port == tcp_si.local_port &&
                    conn.remote_addr == tcp_si.remote_addr &&
                    conn.remote_port == tcp_si.remote_port
                });
                
                match conn_exists {
                    Some((id, _)) => {
                        let conn_id = *id;
                        seen_connections.insert(conn_id);
                        
                        if let Some(conn) = self.connections.get_mut(&conn_id) {
                            conn.update_state(tcp_si.state);
                        }
                    },
                    None => {
                        let new_conn = Connection::new(
                            pid,
                            tcp_si.local_port,
                            tcp_si.remote_port,
                            tcp_si.remote_addr,
                            remote_hostname.clone(),
                            tcp_si.state,
                        );
                        
                        seen_connections.insert(new_conn.id);
                        self.connections.insert(new_conn.id, new_conn);
                        
                        *self.metrics.total_connections_by_pid.entry(pid).or_insert(0) += 1;
                        *self.metrics.current_concurrent_by_pid.entry(pid).or_insert(0) += 1;
                        
                        let current_count = self.metrics.current_concurrent_by_pid[&pid];
                        let max_entry = self.metrics.max_concurrent_by_pid.entry(pid).or_insert(0);
                        if current_count > *max_entry {
                            *max_entry = current_count;
                        }
                        
                        // Update host metrics
                        if let Some(hostname) = &remote_hostname {
                            let host_key = format!("{}:{}", hostname, tcp_si.remote_port);
                            *self.metrics.total_connections_by_host.entry(host_key.clone()).or_insert(0) += 1;
                            *self.metrics.current_concurrent_by_host.entry(host_key.clone()).or_insert(0) += 1;
                            
                            let current_host_count = self.metrics.current_concurrent_by_host[&host_key];
                            let max_host_entry = self.metrics.max_concurrent_by_host.entry(host_key).or_insert(0);
                            if current_host_count > *max_host_entry {
                                *max_host_entry = current_host_count;
                            }
                        }
                        
                        // Update process-host combination metrics
                        if let Some(hostname) = &remote_hostname {
                            let process_host_key = (pid, hostname.clone(), tcp_si.remote_port);
                            *self.metrics.total_connections_by_process_host.entry(process_host_key.clone()).or_insert(0) += 1;
                            *self.metrics.current_concurrent_by_process_host.entry(process_host_key.clone()).or_insert(0) += 1;
                            
                            let current_ph_count = self.metrics.current_concurrent_by_process_host[&process_host_key];
                            let max_ph_entry = self.metrics.max_concurrent_by_process_host.entry(process_host_key).or_insert(0);
                            if current_ph_count > *max_ph_entry {
                                *max_ph_entry = current_ph_count;
                            }
                        }
                    }
                }
                
                // Update process information
                self.update_process_info(pid);
            }
        }
        
        let to_close: Vec<u64> = self.connections.iter()
            .filter(|(id, conn)| !seen_connections.contains(id) && !conn.closed)
            .map(|(id, _)| *id)
            .collect();
            
        for conn_id in to_close {
            if let Some(conn) = self.connections.get_mut(&conn_id) {
                conn.mark_closed();
                
                *self.metrics.current_concurrent_by_pid.entry(conn.pid).or_insert(1) -= 1;
                
                if let Some(hostname) = &conn.remote_hostname {
                    let host_key = format!("{}:{}", hostname, conn.remote_port);
                    *self.metrics.current_concurrent_by_host.entry(host_key).or_insert(1) -= 1;
                    
                    // Update process-host combination metrics
                    let process_host_key = (conn.pid, hostname.clone(), conn.remote_port);
                    *self.metrics.current_concurrent_by_process_host.entry(process_host_key).or_insert(1) -= 1;
                }
                
                // Move to historical connections
                let conn_clone = conn.clone();
                self.historical_connections.push(conn_clone);
            }
        }
        
        // Store the timestamp for historical analysis
        self.metrics.sample_timestamps.push(now);
        
        // Trim timestamp history if it gets too large (keep last 1000 points)
        if self.metrics.sample_timestamps.len() > 1000 {
            self.metrics.sample_timestamps.remove(0);
        }
        
        self.last_refresh = now;
        Ok(())
    }
    
    fn update_process_info(&mut self, pid: u32) {
        if let Some(proc) = self.system_info.process(Pid::from(pid as usize)) {
            let name = proc.name().to_string_lossy().to_string();
            let exe = proc.exe().map(|p| p.to_string_lossy().to_string());
            let memory_usage = proc.memory();
            
            if let Some(process) = self.processes.get_mut(&pid) {
                process.update(Some(name), exe, memory_usage);
            } else {
                let new_process = Process::new(pid, Some(name), exe, memory_usage);
                self.processes.insert(pid, new_process);
            }
            
            let memory_entry = self.metrics.memory_history.entry(pid).or_insert_with(Vec::new);
            memory_entry.push((SystemTime::now(), memory_usage));
            
            // Trim memory history if it gets too large
            if memory_entry.len() > 1000 {
                memory_entry.remove(0);
            }
        }
    }
    
    pub fn get_active_connections(&self) -> Vec<&Connection> {
        self.connections.values()
            .filter(|conn| !conn.closed)
            .collect()
    }
    
    pub fn get_filtered_active_connections(&self, filter: &ConnectionFilter) -> Vec<&Connection> {
        self.connections.values()
            .filter(|conn| !conn.closed)
            .filter(|conn| {
                let process_name = self.get_process(conn.pid)
                    .and_then(|p| p.name.as_deref());
                filter.matches_connection(conn, process_name)
            })
            .collect()
    }
    
    pub fn get_historical_connections(&self) -> &Vec<Connection> {
        &self.historical_connections
    }
    
    pub fn get_filtered_historical_connections(&self, filter: &ConnectionFilter) -> Vec<&Connection> {
        self.historical_connections.iter()
            .filter(|conn| {
                let process_name = self.get_process(conn.pid)
                    .and_then(|p| p.name.as_deref());
                filter.matches_connection(conn, process_name)
            })
            .collect()
    }
    
    pub fn get_process(&self, pid: u32) -> Option<&Process> {
        self.processes.get(&pid)
    }
    
    pub fn get_processes(&self) -> Vec<&Process> {
        self.processes.values().collect()
    }
    
    pub fn get_filtered_processes(&self, filter: &ConnectionFilter) -> Vec<&Process> {
        self.processes.values()
            .filter(|process| {
                if let Some(pid) = filter.pid {
                    if process.pid != pid {
                        return false;
                    }
                }
                
                if let Some(ref name_filter) = filter.process_name {
                    if let Some(ref name) = process.name {
                        if !name.contains(name_filter) {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }
                
                true
            })
            .collect()
    }
    
    pub fn get_connection_history_filtered(
        &self, 
        filter: &ConnectionFilter,
        start_time: Option<SystemTime>,
        end_time: Option<SystemTime>
    ) -> Vec<(SystemTime, usize)> {
        let all_connections: Vec<&Connection> = self.connections.values()
            .chain(self.historical_connections.iter())
            .collect();
        
        let mut filtered_history = Vec::new();
        
        for &timestamp in &self.metrics.sample_timestamps {
            if let Some(start) = start_time {
                if timestamp < start {
                    continue;
                }
            }
            
            if let Some(end) = end_time {
                if timestamp > end {
                    continue;
                }
            }
            
            let active_count = all_connections.iter()
                .filter(|conn| {
                    let was_active = conn.first_seen <= timestamp && 
                                    (timestamp <= conn.last_seen || !conn.closed);
                    let matches_filter = {
                        let process_name = self.get_process(conn.pid)
                            .and_then(|p| p.name.as_deref());
                        filter.matches_connection(conn, process_name)
                    };
                    
                    was_active && matches_filter
                })
                .count();
                
            filtered_history.push((timestamp, active_count));
        }
        
        filtered_history
    }
    
    pub fn get_memory_history_filtered(
        &self,
        filter: &ConnectionFilter,
        start_time: Option<SystemTime>,
        end_time: Option<SystemTime>
    ) -> HashMap<u32, Vec<(SystemTime, u64)>> {
        let mut result = HashMap::new();
        
        let pids_to_include: Vec<u32> = if let Some(pid) = filter.pid {
            vec![pid]
        } else if let Some(ref process_name) = filter.process_name {
            self.processes.iter()
                .filter(|(_, process)| {
                    if let Some(ref name) = process.name {
                        name.contains(process_name)
                    } else {
                        false
                    }
                })
                .map(|(pid, _)| *pid)
                .collect()
        } else {
            self.metrics.memory_history.keys().cloned().collect()
        };
        
        for pid in pids_to_include {
            if let Some(history) = self.metrics.memory_history.get(&pid) {
                let filtered_history: Vec<(SystemTime, u64)> = history.iter()
                    .filter(|(time, _)| {
                        let after_start = if let Some(start) = start_time {
                            *time >= start
                        } else {
                            true
                        };
                        
                        let before_end = if let Some(end) = end_time {
                            *time <= end
                        } else {
                            true
                        };
                        
                        after_start && before_end
                    })
                    .cloned()
                    .collect();
                
                if !filtered_history.is_empty() {
                    result.insert(pid, filtered_history);
                }
            }
        }
        
        result
    }

    pub fn get_host_metrics(&self, filter: &ConnectionFilter) -> Vec<HostMetrics> {
        let mut host_metrics = Vec::new();
        let mut host_map: HashMap<(String, u16), (usize, usize, usize)> = HashMap::new();
        
        let all_connections: Vec<_> = self.connections.values()
            .chain(self.historical_connections.iter())
            .collect();
        
        for conn in all_connections {
            let process_name = self.get_process(conn.pid).and_then(|p| p.name.as_deref());
            if !filter.matches_connection(conn, process_name) {
                continue;
            }
            
            let host = conn.remote_hostname.clone().unwrap_or_else(|| conn.remote_addr.to_string());
            let key = (host.clone(), conn.remote_port);
            
            let entry = host_map.entry(key).or_insert((0, 0, 0));
            
            entry.1 += 1;
            
            if !conn.closed {
                entry.0 += 1;
            }
        }
        
        // Add max concurrent from metrics
        for ((host, port), (current, total, _)) in host_map {
            let host_key = format!("{}:{}", host, port);
            let max_concurrent = self.metrics.max_concurrent_by_host.get(&host_key).cloned().unwrap_or(0);
            
            host_metrics.push(HostMetrics {
                host,
                port,
                current_connections: current,
                total_connections: total,
                max_concurrent,
            });
        }
        
        host_metrics
    }
    
    pub fn get_process_metrics(&self, filter: &ConnectionFilter) -> Vec<ProcessMetrics> {
        let mut process_metrics = Vec::new();
        let mut process_map: HashMap<u32, (usize, usize)> = HashMap::new();
        
        let active_pids = self.get_active_pids();
        
        let all_connections: Vec<_> = self.connections.values()
            .chain(self.historical_connections.iter())
            .collect();
        
        for conn in all_connections {
            let process_name = self.get_process(conn.pid).and_then(|p| p.name.as_deref());
            if !filter.matches_connection(conn, process_name) {
                continue;
            }
            
            let entry = process_map.entry(conn.pid).or_insert((0, 0));
            
            entry.1 += 1;
            
            if !conn.closed {
                entry.0 += 1;
            }
        }
        
        for (pid, (current, total)) in process_map {
            let process = self.get_process(pid);
            let name = process.and_then(|p| p.name.clone()).unwrap_or_else(|| "Unknown".to_string());
            let max_concurrent = self.metrics.max_concurrent_by_pid.get(&pid).cloned().unwrap_or(0);
            let is_alive = active_pids.contains(&pid);
            
            process_metrics.push(ProcessMetrics {
                pid,
                name,
                current_connections: current,
                total_connections: total,
                max_concurrent,
                is_alive,
            });
        }
        
        process_metrics
    }
    
    pub fn get_process_host_metrics(&self, filter: &ConnectionFilter) -> Vec<ProcessHostMetrics> {
        let mut process_host_metrics = Vec::new();
        let mut process_host_map: HashMap<(u32, String, u16), (usize, usize)> = HashMap::new();
        
        let active_pids = self.get_active_pids();

        let all_connections: Vec<_> = self.connections.values()
            .chain(self.historical_connections.iter())
            .collect();
        
        for conn in all_connections {
            let process_name = self.get_process(conn.pid).and_then(|p| p.name.as_deref());
            if !filter.matches_connection(conn, process_name) {
                continue;
            }
            
            let host = conn.remote_hostname.clone().unwrap_or_else(|| conn.remote_addr.to_string());
            let key = (conn.pid, host.clone(), conn.remote_port);
            
            let entry = process_host_map.entry(key).or_insert((0, 0));
            
            entry.1 += 1;
            
            if !conn.closed {
                entry.0 += 1;
            }
        }
        
        for ((pid, host, port), (current, total)) in process_host_map {
            let process = self.get_process(pid);
            let process_name = process
                .and_then(|p| p.exe.clone().or(p.name.clone()))
                .unwrap_or_else(|| "Unknown".to_string());
            let process_host_key = (pid, host.clone(), port);
            let max_concurrent = self.metrics.max_concurrent_by_process_host.get(&process_host_key).cloned().unwrap_or(0);
            let is_alive = active_pids.contains(&pid);
            
            process_host_metrics.push(ProcessHostMetrics {
                pid,
                process_name,
                host,
                port,
                current_connections: current,
                total_connections: total,
                max_concurrent,
                is_alive,
            });
        }
        
        process_host_metrics
    }

    fn get_active_pids(&self) -> HashSet<u32> {
        self.system_info.processes()
            .iter()
            .filter(|(_, process)| {
                !matches!(process.status(), ProcessStatus::Dead | ProcessStatus::Zombie | ProcessStatus::Stop)
            })
            .map(|(pid, _)| pid.as_u32())
            .collect()
    }
}