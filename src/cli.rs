use clap::{Arg, Command};
use crate::core::filters::ConnectionFilter;

pub fn parse_args() -> ConnectionFilter {
    let matches = Command::new("tcpcount")
        .version("0.1.0")
        .author("Hunter Young")
        .about("Monitor and count TCP connections")
        .arg(
            Arg::new("pid")
                .short('p')
                .long("pid")
                .help("Filter by process ID")
                .value_name("PID")
                .num_args(1)
        )
        .arg(
            Arg::new("process")
                .short('n')
                .long("process-name")
                .help("Filter by process name (case-sensitive substring match)")
                .value_name("NAME")
                .num_args(1)
        )
        .arg(
            Arg::new("host")
                .short('H')
                .long("host")
                .help("Filter by remote host (case-sensitive substring match)")
                .value_name("HOST")
                .num_args(1)
        )
        .arg(
            Arg::new("port")
                .short('P')
                .long("port")
                .help("Filter by remote port")
                .value_name("PORT")
                .num_args(1)
        )
        .get_matches();

    let mut filter = ConnectionFilter::default();
    
    if let Some(pid_str) = matches.get_one::<String>("pid") {
        match pid_str.parse::<u32>() {
            Ok(pid) => filter.pid = Some(pid),
            Err(_) => eprintln!("Warning: Invalid PID '{}', ignoring", pid_str),
        }
    }
    
    if let Some(process_name) = matches.get_one::<String>("process") {
        filter.process_name = Some(process_name.clone());
    }
    
    if let Some(host) = matches.get_one::<String>("host") {
        filter.remote_host = Some(host.clone());
    }
    
    if let Some(port_str) = matches.get_one::<String>("port") {
        match port_str.parse::<u16>() {
            Ok(port) => filter.remote_port = Some(port),
            Err(_) => eprintln!("Warning: Invalid port '{}', ignoring", port_str),
        }
    }
    
    filter
}