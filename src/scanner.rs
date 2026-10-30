use anyhow::Result;
use netstat2::{get_sockets_info, AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo, TcpState};
use std::collections::HashMap;
use sysinfo::{Pid, ProcessesToUpdate, System};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Tcp,
    Udp,
}

impl Protocol {
    pub fn as_str(&self) -> &'static str {
        match self {
            Protocol::Tcp => "TCP",
            Protocol::Udp => "UDP",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PortEntry {
    pub pid: u32,
    pub process_name: String,
    pub protocol: Protocol,
    pub local_addr: String,
    pub local_port: u16,
    pub remote_addr: String,
    pub remote_port: u16,
    pub state: String,
    pub is_listening: bool,
    pub cpu_usage: f32,
    pub memory_bytes: u64,
    pub exe_path: String,
    pub cmd_line: String,
    pub run_time_secs: u64,
    pub disk_read_bytes: u64,
    pub disk_write_bytes: u64,
    pub service_hint: Option<&'static str>,
}

impl PortEntry {
    pub fn memory_mb(&self) -> f64 {
        self.memory_bytes as f64 / (1024.0 * 1024.0)
    }

    pub fn is_dev_port(&self) -> bool {
        matches!(
            self.local_port,
            3000..=3010
                | 4000
                | 4200
                | 5000..=5005
                | 5173..=5175
                | 5432
                | 6379
                | 8000..=8010
                | 8080..=8090
                | 8888
                | 9000
                | 9229
                | 27017
        )
    }
}

#[derive(Debug, Clone)]
pub struct SystemSnapshot {
    pub global_cpu: f32,
    pub per_core_cpu: Vec<f32>,
    pub used_memory: u64,
    pub total_memory: u64,
    pub used_swap: u64,
    pub total_swap: u64,
    pub total_processes: usize,
    pub uptime_secs: u64,
    pub host_name: String,
    pub os_name: String,
    pub entries: Vec<PortEntry>,
}

pub struct SystemScanner {
    sys: System,
    cpu_count: usize,
}

impl SystemScanner {
    pub fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();
        let cpu_count = sys.cpus().len().max(1);
        Self { sys, cpu_count }
    }

    pub fn scan(&mut self) -> Result<SystemSnapshot> {
        self.sys.refresh_cpu_all();
        self.sys.refresh_memory();
        self.sys.refresh_processes(ProcessesToUpdate::All, true);

        let global_cpu = self.sys.global_cpu_usage();
        let per_core_cpu: Vec<f32> = self.sys.cpus().iter().map(|c| c.cpu_usage()).collect();
        let used_memory = self.sys.used_memory();
        let total_memory = self.sys.total_memory().max(1);
        let used_swap = self.sys.used_swap();
        let total_swap = self.sys.total_swap();
        let total_processes = self.sys.processes().len();
        let uptime_secs = System::uptime();
        let host_name = System::host_name().unwrap_or_else(|| "localhost".to_string());
        let os_name = System::name().unwrap_or_else(|| std::env::consts::OS.to_string());

        let af_flags = AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6;
        let proto_flags = ProtocolFlags::TCP | ProtocolFlags::UDP;
        let sockets = get_sockets_info(af_flags, proto_flags)?;

        let mut entries = Vec::with_capacity(sockets.len());
        let mut seen = HashMap::new();

        for si in sockets {
            let pid = si.associated_pids.first().copied().unwrap_or(0);
            let (protocol, local_addr, local_port, remote_addr, remote_port, state, is_listening) =
                match si.protocol_socket_info {
                    ProtocolSocketInfo::Tcp(tcp) => {
                        let listening = tcp.state == TcpState::Listen;
                        (
                            Protocol::Tcp,
                            tcp.local_addr.to_string(),
                            tcp.local_port,
                            tcp.remote_addr.to_string(),
                            tcp.remote_port,
                            tcp.state.to_string(),
                            listening,
                        )
                    }
                    ProtocolSocketInfo::Udp(udp) => (
                        Protocol::Udp,
                        udp.local_addr.to_string(),
                        udp.local_port,
                        "*".to_string(),
                        0,
                        "STATELESS".to_string(),
                        true,
                    ),
                };

            // Deduplicate identical (pid, proto, local_port, state) when bound on both IPv4 & IPv6
            let dedup_key = (pid, protocol == Protocol::Tcp, local_port, is_listening, remote_port);
            if seen.contains_key(&dedup_key) && is_listening {
                continue;
            }
            seen.insert(dedup_key, true);

            let (
                process_name,
                cpu_usage,
                memory_bytes,
                exe_path,
                cmd_line,
                run_time_secs,
                disk_read_bytes,
                disk_write_bytes,
            ) = if pid > 0 {
                if let Some(proc_info) = self.sys.process(Pid::from_u32(pid)) {
                    let name = proc_info.name().to_string_lossy().to_string();
                    let cpu = proc_info.cpu_usage() / (self.cpu_count as f32);
                    let mem = proc_info.memory();
                    let exe = proc_info
                        .exe()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|| "-".to_string());
                    let cmd_vec: Vec<String> = proc_info
                        .cmd()
                        .iter()
                        .map(|s| s.to_string_lossy().to_string())
                        .collect();
                    let cmd = if cmd_vec.is_empty() {
                        exe.clone()
                    } else {
                        cmd_vec.join(" ")
                    };
                    let du = proc_info.disk_usage();
                    (
                        name,
                        cpu,
                        mem,
                        exe,
                        cmd,
                        proc_info.run_time(),
                        du.total_read_bytes,
                        du.total_written_bytes,
                    )
                } else {
                    (
                        "System / Protected".to_string(),
                        0.0,
                        0,
                        "-".to_string(),
                        "-".to_string(),
                        0,
                        0,
                        0,
                    )
                }
            } else {
                (
                    "Kernel / Idle".to_string(),
                    0.0,
                    0,
                    "-".to_string(),
                    "-".to_string(),
                    0,
                    0,
                    0,
                )
            };

            let service_hint = well_known_service(local_port);

            entries.push(PortEntry {
                pid,
                process_name,
                protocol,
                local_addr,
                local_port,
                remote_addr,
                remote_port,
                state,
                is_listening,
                cpu_usage,
                memory_bytes,
                exe_path,
                cmd_line,
                run_time_secs,
                disk_read_bytes,
                disk_write_bytes,
                service_hint,
            });
        }

        Ok(SystemSnapshot {
            global_cpu,
            per_core_cpu,
            used_memory,
            total_memory,
            used_swap,
            total_swap,
            total_processes,
            uptime_secs,
            host_name,
            os_name,
            entries,
        })
    }

    pub fn kill_process(&mut self, pid: u32) -> Result<String> {
        if pid == 0 || pid == 4 {
            anyhow::bail!("Cannot terminate system kernel process (PID {}).", pid);
        }
        self.sys.refresh_processes(ProcessesToUpdate::All, true);
        if let Some(process) = self.sys.process(Pid::from_u32(pid)) {
            let name = process.name().to_string_lossy().to_string();
            if process.kill() {
                Ok(format!("Terminated {} (PID {})", name, pid))
            } else {
                anyhow::bail!(
                    "Failed to kill {} (PID {}). Try running as Administrator/root.",
                    name,
                    pid
                )
            }
        } else {
            anyhow::bail!("Process PID {} is no longer running.", pid)
        }
    }
}

pub fn well_known_service(port: u16) -> Option<&'static str> {
    match port {
        21 => Some("FTP"),
        22 => Some("SSH"),
        25 => Some("SMTP"),
        53 => Some("DNS"),
        80 => Some("HTTP"),
        135 => Some("RPC"),
        139 => Some("NetBIOS"),
        443 => Some("HTTPS"),
        445 => Some("SMB"),
        1433 => Some("MSSQL"),
        3000..=3005 => Some("Node/React Dev"),
        3306 => Some("MySQL"),
        3389 => Some("RDP"),
        4000 => Some("Dev Server"),
        4200 => Some("Angular Dev"),
        5000..=5001 => Some("Flask/.NET"),
        5173..=5175 => Some("Vite Dev"),
        5432 => Some("PostgreSQL"),
        5672 => Some("RabbitMQ"),
        6379 => Some("Redis"),
        8000..=8008 => Some("HTTP Dev"),
        8080..=8085 => Some("HTTP Proxy/Dev"),
        8443 => Some("HTTPS Alt"),
        8888 => Some("Jupyter"),
        9000 => Some("PHP/Sonar/MinIO"),
        9090 => Some("Prometheus"),
        9200 => Some("Elasticsearch"),
        9229 => Some("Node Inspector"),
        11434 => Some("Ollama AI"),
        27017 => Some("MongoDB"),
        _ => None,
    }
}
