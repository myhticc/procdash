use crate::scanner::{PortEntry, Protocol, SystemScanner, SystemSnapshot};
use ratatui::widgets::TableState;
use std::collections::VecDeque;
use std::time::Instant;

const HISTORY_LEN: usize = 90;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    EditingSearch,
    ConfirmKill,
    ShowHelp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortColumn {
    Port,
    Process,
    Pid,
    Cpu,
    Memory,
}

impl SortColumn {
    pub fn next(self) -> Self {
        match self {
            SortColumn::Port => SortColumn::Cpu,
            SortColumn::Cpu => SortColumn::Memory,
            SortColumn::Memory => SortColumn::Pid,
            SortColumn::Pid => SortColumn::Process,
            SortColumn::Process => SortColumn::Port,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SortColumn::Port => "PORT",
            SortColumn::Process => "PROCESS",
            SortColumn::Pid => "PID",
            SortColumn::Cpu => "CPU%",
            SortColumn::Memory => "RAM",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolFilter {
    All,
    Tcp,
    Udp,
}

impl ProtocolFilter {
    pub fn next(self) -> Self {
        match self {
            ProtocolFilter::All => ProtocolFilter::Tcp,
            ProtocolFilter::Tcp => ProtocolFilter::Udp,
            ProtocolFilter::Udp => ProtocolFilter::All,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ProtocolFilter::All => "TCP+UDP",
            ProtocolFilter::Tcp => "TCP Only",
            ProtocolFilter::Udp => "UDP Only",
        }
    }
}

pub struct App {
    pub scanner: SystemScanner,
    pub snapshot: SystemSnapshot,
    pub filtered_entries: Vec<PortEntry>,
    pub table_state: TableState,
    pub input_mode: InputMode,
    pub search_query: String,
    pub listen_only: bool,
    pub dev_only: bool,
    pub proto_filter: ProtocolFilter,
    pub sort_by: SortColumn,
    pub sort_desc: bool,
    pub paused: bool,
    pub cpu_history: VecDeque<u64>,
    pub mem_history: VecDeque<u64>,
    pub status_message: Option<(String, bool, Instant)>,
    pub should_quit: bool,
    pub tick_count: u64,
}

impl App {
    pub fn new(initial_search: Option<String>, all_states: bool) -> anyhow::Result<Self> {
        let mut scanner = SystemScanner::new();
        let snapshot = scanner.scan()?;

        let mut cpu_history = VecDeque::with_capacity(HISTORY_LEN);
        let mut mem_history = VecDeque::with_capacity(HISTORY_LEN);
        for _ in 0..HISTORY_LEN {
            cpu_history.push_back(0);
            mem_history.push_back(0);
        }

        let mut table_state = TableState::default();
        table_state.select(Some(0));

        let mut app = Self {
            scanner,
            snapshot,
            filtered_entries: Vec::new(),
            table_state,
            input_mode: InputMode::Normal,
            search_query: initial_search.unwrap_or_default(),
            listen_only: !all_states,
            dev_only: false,
            proto_filter: ProtocolFilter::All,
            sort_by: SortColumn::Port,
            sort_desc: false,
            paused: false,
            cpu_history,
            mem_history,
            status_message: None,
            should_quit: false,
            tick_count: 0,
        };

        app.push_metrics();
        app.apply_filters();
        Ok(app)
    }

    pub fn refresh(&mut self) {
        if self.paused {
            return;
        }
        self.tick_count = self.tick_count.wrapping_add(1);
        if let Ok(snap) = self.scanner.scan() {
            self.snapshot = snap;
            self.push_metrics();
            self.apply_filters();
        }
    }

    fn push_metrics(&mut self) {
        let cpu_val = self.snapshot.global_cpu.round().clamp(0.0, 100.0) as u64;
        let mem_pct = if self.snapshot.total_memory > 0 {
            ((self.snapshot.used_memory as f64 / self.snapshot.total_memory as f64) * 100.0)
                .round()
                .clamp(0.0, 100.0) as u64
        } else {
            0
        };

        if self.cpu_history.len() >= HISTORY_LEN {
            self.cpu_history.pop_front();
        }
        self.cpu_history.push_back(cpu_val);

        if self.mem_history.len() >= HISTORY_LEN {
            self.mem_history.pop_front();
        }
        self.mem_history.push_back(mem_pct);
    }

    pub fn apply_filters(&mut self) {
        let query = self.search_query.trim().to_lowercase();

        let mut list: Vec<PortEntry> = self
            .snapshot
            .entries
            .iter()
            .filter(|e| {
                if self.listen_only && !e.is_listening {
                    return false;
                }
                if self.dev_only && !e.is_dev_port() {
                    return false;
                }
                match self.proto_filter {
                    ProtocolFilter::All => {}
                    ProtocolFilter::Tcp => {
                        if e.protocol != Protocol::Tcp {
                            return false;
                        }
                    }
                    ProtocolFilter::Udp => {
                        if e.protocol != Protocol::Udp {
                            return false;
                        }
                    }
                }
                if !query.is_empty() {
                    let port_str = e.local_port.to_string();
                    let pid_str = e.pid.to_string();
                    let name_match = e.process_name.to_lowercase().contains(&query);
                    let port_match = port_str.contains(&query);
                    let pid_match = pid_str.contains(&query);
                    let hint_match = e
                        .service_hint
                        .map(|h| h.to_lowercase().contains(&query))
                        .unwrap_or(false);
                    let cmd_match = e.cmd_line.to_lowercase().contains(&query);
                    if !(name_match || port_match || pid_match || hint_match || cmd_match) {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        list.sort_by(|a, b| {
            let ord = match self.sort_by {
                SortColumn::Port => a.local_port.cmp(&b.local_port),
                SortColumn::Process => a
                    .process_name
                    .to_lowercase()
                    .cmp(&b.process_name.to_lowercase()),
                SortColumn::Pid => a.pid.cmp(&b.pid),
                SortColumn::Cpu => a
                    .cpu_usage
                    .partial_cmp(&b.cpu_usage)
                    .unwrap_or(std::cmp::Ordering::Equal),
                SortColumn::Memory => a.memory_bytes.cmp(&b.memory_bytes),
            };
            if self.sort_desc {
                ord.reverse()
            } else {
                ord
            }
        });

        self.filtered_entries = list;

        // Keep selection valid
        if self.filtered_entries.is_empty() {
            self.table_state.select(None);
        } else {
            let current = self.table_state.selected().unwrap_or(0);
            if current >= self.filtered_entries.len() {
                self.table_state
                    .select(Some(self.filtered_entries.len() - 1));
            } else if self.table_state.selected().is_none() {
                self.table_state.select(Some(0));
            }
        }
    }

    pub fn selected_entry(&self) -> Option<&PortEntry> {
        self.table_state
            .selected()
            .and_then(|idx| self.filtered_entries.get(idx))
    }

    pub fn next_row(&mut self) {
        if self.filtered_entries.is_empty() {
            return;
        }
        let i = match self.table_state.selected() {
            Some(i) => {
                if i >= self.filtered_entries.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    pub fn previous_row(&mut self) {
        if self.filtered_entries.is_empty() {
            return;
        }
        let i = match self.table_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.filtered_entries.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    pub fn page_down(&mut self) {
        if self.filtered_entries.is_empty() {
            return;
        }
        let i = self.table_state.selected().unwrap_or(0);
        let next = (i + 10).min(self.filtered_entries.len() - 1);
        self.table_state.select(Some(next));
    }

    pub fn page_up(&mut self) {
        if self.filtered_entries.is_empty() {
            return;
        }
        let i = self.table_state.selected().unwrap_or(0);
        let prev = i.saturating_sub(10);
        self.table_state.select(Some(prev));
    }

    pub fn kill_selected(&mut self) {
        if let Some(entry) = self.selected_entry().cloned() {
            match self.scanner.kill_process(entry.pid) {
                Ok(msg) => {
                    self.set_status(
                        format!("{} (Port :{})", msg, entry.local_port),
                        true,
                    );
                    if let Ok(snap) = self.scanner.scan() {
                        self.snapshot = snap;
                        self.apply_filters();
                    }
                }
                Err(err) => {
                    self.set_status(err.to_string(), false);
                }
            }
        }
    }

    pub fn set_status(&mut self, msg: String, is_success: bool) {
        self.status_message = Some((msg, is_success, Instant::now()));
    }

    pub fn active_status(&self) -> Option<(&str, bool)> {
        if let Some((ref msg, ok, ts)) = self.status_message {
            if ts.elapsed().as_secs() < 6 {
                return Some((msg.as_str(), ok));
            }
        }
        None
    }

    pub fn sibling_ports_for_pid(&self, pid: u32) -> Vec<u16> {
        if pid == 0 {
            return Vec::new();
        }
        let mut ports: Vec<u16> = self
            .snapshot
            .entries
            .iter()
            .filter(|e| e.pid == pid)
            .map(|e| e.local_port)
            .collect();
        ports.sort_unstable();
        ports.dedup();
        ports
    }
}
