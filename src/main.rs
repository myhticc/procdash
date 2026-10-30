mod app;
mod scanner;
mod ui;

use anyhow::Result;
use app::{App, InputMode};
use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use scanner::SystemScanner;
use std::{
    io,
    time::{Duration, Instant},
};

#[derive(Parser, Debug)]
#[command(
    name = "procdash",
    version,
    about = "⚡ PortKiller & Process Telemetry TUI/CLI written in Rust",
    long_about = "Inspect active TCP/UDP ports, monitor real-time CPU & RAM usage with sparklines, and kill port-hogging processes with a single keystroke."
)]
struct Cli {
    /// Pre-filter TUI or CLI output by port number, process name, or PID
    #[arg(short, long)]
    filter: Option<String>,

    /// Inspect a specific port and exit (CLI mode)
    #[arg(short, long)]
    port: Option<u16>,

    /// Immediately kill the process listening on the specified port and exit (CLI mode)
    #[arg(short, long)]
    kill: Option<u16>,

    /// Print active ports table to stdout and exit (CLI mode)
    #[arg(short, long)]
    list: bool,

    /// Include all socket states (ESTABLISHED, TIME_WAIT, etc.) by default
    #[arg(short, long)]
    all: bool,

    /// Refresh interval in milliseconds for TUI telemetry
    #[arg(short, long, default_value_t = 1000)]
    interval: u64,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // CLI Mode: --kill <PORT>
    if let Some(target_port) = cli.kill {
        return cli_kill_port(target_port);
    }

    // CLI Mode: --port <PORT>
    if let Some(target_port) = cli.port {
        return cli_inspect_port(target_port);
    }

    // CLI Mode: --list
    if cli.list {
        return cli_list_ports(cli.filter.as_deref(), cli.all);
    }

    // Interactive TUI Mode
    run_tui(cli.filter, cli.all, cli.interval)
}

fn cli_kill_port(port: u16) -> Result<()> {
    let mut scanner = SystemScanner::new();
    let snap = scanner.scan()?;
    let matches: Vec<_> = snap
        .entries
        .iter()
        .filter(|e| e.local_port == port && e.pid > 0)
        .collect();

    if matches.is_empty() {
        println!("No active process found bound to port :{}.", port);
        return Ok(());
    }

    let mut killed_pids = Vec::new();
    for entry in matches {
        if killed_pids.contains(&entry.pid) {
            continue;
        }
        killed_pids.push(entry.pid);
        match scanner.kill_process(entry.pid) {
            Ok(msg) => println!("✔ {} on port :{} ({})", msg, port, entry.protocol.as_str()),
            Err(err) => eprintln!("✖ {}", err),
        }
    }
    Ok(())
}

fn cli_inspect_port(port: u16) -> Result<()> {
    let mut scanner = SystemScanner::new();
    let snap = scanner.scan()?;
    let matches: Vec<_> = snap
        .entries
        .iter()
        .filter(|e| e.local_port == port)
        .collect();

    if matches.is_empty() {
        println!("Port :{} is currently free (no active sockets).", port);
        return Ok(());
    }

    println!(
        "{:<8} {:<6} {:<8} {:<22} {:<8} {:<10} {:<12} {}",
        "PORT", "PROTO", "PID", "PROCESS", "CPU%", "RAM(MB)", "STATE", "COMMAND / PATH"
    );
    println!("{}", "-".repeat(95));
    for e in matches {
        println!(
            ":{:<7} {:<6} {:<8} {:<22} {:>6.1}% {:>9.1} {:<12} {}",
            e.local_port,
            e.protocol.as_str(),
            e.pid,
            truncate_str(&e.process_name, 21),
            e.cpu_usage,
            e.memory_mb(),
            e.state,
            truncate_str(&e.cmd_line, 40)
        );
    }
    Ok(())
}

fn cli_list_ports(filter: Option<&str>, show_all: bool) -> Result<()> {
    let app = App::new(filter.map(|s| s.to_string()), show_all)?;
    println!(
        "⚡ PROCDASH SNAPSHOT — Host: {} ({}) | CPU: {:.1}% | RAM: {:.1}/{:.1} GB",
        app.snapshot.host_name,
        app.snapshot.os_name,
        app.snapshot.global_cpu,
        app.snapshot.used_memory as f64 / 1073741824.0,
        app.snapshot.total_memory as f64 / 1073741824.0,
    );
    println!("{}", "=".repeat(96));
    println!(
        "{:<8} {:<6} {:<15} {:<8} {:<22} {:>7} {:>10}  {:<12}",
        "PORT", "PROTO", "SERVICE", "PID", "PROCESS", "CPU%", "RAM(MB)", "STATE"
    );
    println!("{}", "-".repeat(96));

    for e in &app.filtered_entries {
        let dev_marker = if e.is_dev_port() { "★" } else { " " };
        println!(
            "{}:{:<6} {:<6} {:<15} {:<8} {:<22} {:>6.1}% {:>9.1}  {:<12}",
            dev_marker,
            e.local_port,
            e.protocol.as_str(),
            e.service_hint.unwrap_or("-"),
            e.pid,
            truncate_str(&e.process_name, 21),
            e.cpu_usage,
            e.memory_mb(),
            e.state
        );
    }
    println!(
        "\nTotal displayed: {} sockets (of {} total)",
        app.filtered_entries.len(),
        app.snapshot.entries.len()
    );
    Ok(())
}

fn truncate_str(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars {
        s.to_string()
    } else {
        let taken: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", taken)
    }
}

fn run_tui(initial_filter: Option<String>, all_states: bool, interval_ms: u64) -> Result<()> {
    let mut app = App::new(initial_filter, all_states)?;

    // Setup panic hook so terminal is restored cleanly on panic
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        original_hook(panic_info);
    }));

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let tick_rate = Duration::from_millis(interval_ms.max(250));
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| ui::draw(f, &mut app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                // Ignore key release events on Windows
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                // Global Ctrl+C exit
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    app.should_quit = true;
                }

                match app.input_mode {
                    InputMode::Normal => match key.code {
                        KeyCode::Char('q') => app.should_quit = true,
                        KeyCode::Down | KeyCode::Char('j') => app.next_row(),
                        KeyCode::Up | KeyCode::Char('u') => app.previous_row(),
                        KeyCode::PageDown => app.page_down(),
                        KeyCode::PageUp => app.page_up(),
                        KeyCode::Char('/') => {
                            app.input_mode = InputMode::EditingSearch;
                        }
                        KeyCode::Esc => {
                            if !app.search_query.is_empty() {
                                app.search_query.clear();
                                app.apply_filters();
                            }
                        }
                        KeyCode::Char('k') | KeyCode::Delete => {
                            if app.selected_entry().is_some() {
                                app.input_mode = InputMode::ConfirmKill;
                            }
                        }
                        KeyCode::Char('l') => {
                            app.listen_only = !app.listen_only;
                            app.apply_filters();
                        }
                        KeyCode::Char('d') => {
                            app.dev_only = !app.dev_only;
                            app.apply_filters();
                        }
                        KeyCode::Char('p') => {
                            app.proto_filter = app.proto_filter.next();
                            app.apply_filters();
                        }
                        KeyCode::Char('s') | KeyCode::Tab => {
                            app.sort_by = app.sort_by.next();
                            app.apply_filters();
                        }
                        KeyCode::Char('r') => {
                            app.sort_desc = !app.sort_desc;
                            app.apply_filters();
                        }
                        KeyCode::Char(' ') => {
                            app.paused = !app.paused;
                        }
                        KeyCode::Char('?') | KeyCode::F(1) => {
                            app.input_mode = InputMode::ShowHelp;
                        }
                        _ => {}
                    },
                    InputMode::EditingSearch => match key.code {
                        KeyCode::Enter | KeyCode::Down | KeyCode::Up => {
                            app.input_mode = InputMode::Normal;
                        }
                        KeyCode::Esc => {
                            app.search_query.clear();
                            app.apply_filters();
                            app.input_mode = InputMode::Normal;
                        }
                        KeyCode::Backspace => {
                            app.search_query.pop();
                            app.apply_filters();
                        }
                        KeyCode::Char(c) => {
                            app.search_query.push(c);
                            app.apply_filters();
                        }
                        _ => {}
                    },
                    InputMode::ConfirmKill => match key.code {
                        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                            app.kill_selected();
                            app.input_mode = InputMode::Normal;
                        }
                        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                            app.input_mode = InputMode::Normal;
                        }
                        _ => {}
                    },
                    InputMode::ShowHelp => match key.code {
                        KeyCode::Esc | KeyCode::Enter | KeyCode::Char('?') | KeyCode::Char('q') => {
                            app.input_mode = InputMode::Normal;
                        }
                        _ => {}
                    },
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            app.refresh();
            last_tick = Instant::now();
        }

        if app.should_quit {
            break;
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
