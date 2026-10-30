use crate::app::{App, InputMode, SortColumn};
use crate::scanner::Protocol;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Cell, Clear, Gauge, Paragraph, Row, Sparkline, Table, Wrap,
    },
    Frame,
};

pub fn draw(f: &mut Frame, app: &mut App) {
    let size = f.area();

    let root_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Top header bar
            Constraint::Length(7), // CPU + RAM telemetry sparklines & gauges
            Constraint::Length(3), // Search & Filter bar
            Constraint::Min(10),   // Main table + Process inspector
            Constraint::Length(2), // Footer / Status toast
        ])
        .split(size);

    draw_header(f, app, root_chunks[0]);
    draw_telemetry(f, app, root_chunks[1]);
    draw_filter_bar(f, app, root_chunks[2]);
    draw_main_body(f, app, root_chunks[3]);
    draw_footer(f, app, root_chunks[4]);

    match app.input_mode {
        InputMode::ConfirmKill => draw_kill_modal(f, app, size),
        InputMode::ShowHelp => draw_help_modal(f, size),
        _ => {}
    }
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let live_badge = if app.paused {
        Span::styled(
            " ⏸ PAUSED ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        let dot = if app.tick_count % 2 == 0 { "●" } else { "○" };
        Span::styled(
            format!(" {} LIVE ", dot),
            Style::default()
                .fg(Color::Black)
                .bg(Color::Rgb(0, 230, 150))
                .add_modifier(Modifier::BOLD),
        )
    };

    let uptime_hrs = app.snapshot.uptime_secs / 3600;
    let uptime_mins = (app.snapshot.uptime_secs % 3600) / 60;

    let header_line = Line::from(vec![
        Span::styled(
            " ⚡ PROCDASH // PORTKILLER ",
            Style::default()
                .fg(Color::Rgb(80, 220, 255))
                .add_modifier(Modifier::BOLD),
        ),
        live_badge,
        Span::raw("  "),
        Span::styled("Host: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            &app.snapshot.host_name,
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" ({})  ", app.snapshot.os_name),
            Style::default().fg(Color::Gray),
        ),
        Span::styled("Procs: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            app.snapshot.total_processes.to_string(),
            Style::default().fg(Color::Cyan),
        ),
        Span::raw("  "),
        Span::styled("Sockets: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!(
                "{}/{}",
                app.filtered_entries.len(),
                app.snapshot.entries.len()
            ),
            Style::default()
                .fg(Color::Rgb(255, 200, 80))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled("Uptime: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{}h {:02}m", uptime_hrs, uptime_mins),
            Style::default().fg(Color::Gray),
        ),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(45, 75, 110)));

    let p = Paragraph::new(header_line).block(block);
    f.render_widget(p, area);
}

fn draw_telemetry(f: &mut Frame, app: &App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // --- CPU Card ---
    let cpu_pct = app.snapshot.global_cpu.clamp(0.0, 100.0);
    let cpu_color = if cpu_pct > 80.0 {
        Color::Red
    } else if cpu_pct > 50.0 {
        Color::Yellow
    } else {
        Color::Rgb(0, 215, 255)
    };

    let cpu_block = Block::default()
        .title(Span::styled(
            format!(
                " CPU LOAD [{:5.1}%] ({} Cores) ",
                cpu_pct,
                app.snapshot.per_core_cpu.len()
            ),
            Style::default().fg(cpu_color).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(45, 75, 110)));

    let inner_cpu = cpu_block.inner(cols[0]);
    f.render_widget(cpu_block, cols[0]);

    let cpu_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(2)])
        .split(inner_cpu);

    let cpu_gauge = Gauge::default()
        .gauge_style(
            Style::default()
                .fg(cpu_color)
                .bg(Color::Rgb(25, 30, 40))
                .add_modifier(Modifier::BOLD),
        )
        .percent(cpu_pct.round() as u16)
        .label(format!("{:.1}% Total CPU", cpu_pct));
    f.render_widget(cpu_gauge, cpu_rows[0]);

    let cpu_data: Vec<u64> = app.cpu_history.iter().copied().collect();
    let cpu_spark = Sparkline::default()
        .data(&cpu_data)
        .max(100)
        .style(Style::default().fg(cpu_color));
    f.render_widget(cpu_spark, cpu_rows[1]);

    // --- Memory Card ---
    let used_gb = app.snapshot.used_memory as f64 / (1024.0 * 1024.0 * 1024.0);
    let total_gb = app.snapshot.total_memory as f64 / (1024.0 * 1024.0 * 1024.0);
    let mem_pct = if app.snapshot.total_memory > 0 {
        ((app.snapshot.used_memory as f64 / app.snapshot.total_memory as f64) * 100.0)
            .clamp(0.0, 100.0)
    } else {
        0.0
    };

    let mem_color = if mem_pct > 85.0 {
        Color::Red
    } else if mem_pct > 65.0 {
        Color::Yellow
    } else {
        Color::Rgb(175, 120, 255)
    };

    let mem_block = Block::default()
        .title(Span::styled(
            format!(
                " MEMORY USAGE [{:.1} / {:.1} GB ({:.0}%)] ",
                used_gb, total_gb, mem_pct
            ),
            Style::default().fg(mem_color).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(45, 75, 110)));

    let inner_mem = mem_block.inner(cols[1]);
    f.render_widget(mem_block, cols[1]);

    let mem_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(2)])
        .split(inner_mem);

    let swap_used_gb = app.snapshot.used_swap as f64 / (1024.0 * 1024.0 * 1024.0);
    let swap_total_gb = app.snapshot.total_swap as f64 / (1024.0 * 1024.0 * 1024.0);
    let mem_gauge = Gauge::default()
        .gauge_style(
            Style::default()
                .fg(mem_color)
                .bg(Color::Rgb(25, 30, 40))
                .add_modifier(Modifier::BOLD),
        )
        .percent(mem_pct.round() as u16)
        .label(format!(
            "{:.2} GB Used  |  Swap: {:.2}/{:.2} GB",
            used_gb, swap_used_gb, swap_total_gb
        ));
    f.render_widget(mem_gauge, mem_rows[0]);

    let mem_data: Vec<u64> = app.mem_history.iter().copied().collect();
    let mem_spark = Sparkline::default()
        .data(&mem_data)
        .max(100)
        .style(Style::default().fg(mem_color));
    f.render_widget(mem_spark, mem_rows[1]);
}

fn draw_filter_bar(f: &mut Frame, app: &App, area: Rect) {
    let is_searching = app.input_mode == InputMode::EditingSearch;

    let search_prefix = if is_searching {
        Span::styled(
            " 🔎 SEARCH (typing): ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(
            " 🔎 Filter [/]: ",
            Style::default().fg(Color::Rgb(120, 180, 240)),
        )
    };

    let query_text = if app.search_query.is_empty() && !is_searching {
        Span::styled(
            "port, process, PID, service...",
            Style::default().fg(Color::DarkGray),
        )
    } else {
        let cursor = if is_searching { "█" } else { "" };
        Span::styled(
            format!("{}{}", app.search_query, cursor),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
    };

    let listen_pill = if app.listen_only {
        Span::styled(
            " [l] LISTEN ONLY ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Rgb(80, 210, 255))
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(
            " [l] ALL STATES ",
            Style::default().fg(Color::Gray).bg(Color::Rgb(35, 40, 52)),
        )
    };

    let dev_pill = if app.dev_only {
        Span::styled(
            " [d] ★ DEV PORTS ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Rgb(255, 195, 50))
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(
            " [d] All Ports ",
            Style::default().fg(Color::Gray).bg(Color::Rgb(35, 40, 52)),
        )
    };

    let proto_pill = Span::styled(
        format!(" [p] {} ", app.proto_filter.label()),
        Style::default()
            .fg(Color::Rgb(200, 220, 255))
            .bg(Color::Rgb(40, 55, 80)),
    );

    let arrow = if app.sort_desc { "▼" } else { "▲" };
    let sort_pill = Span::styled(
        format!(" [s/r] Sort: {} {} ", app.sort_by.label(), arrow),
        Style::default()
            .fg(Color::Rgb(160, 250, 190))
            .bg(Color::Rgb(30, 60, 50)),
    );

    let line = Line::from(vec![
        search_prefix,
        query_text,
        Span::raw("   "),
        listen_pill,
        Span::raw(" "),
        dev_pill,
        Span::raw(" "),
        proto_pill,
        Span::raw(" "),
        sort_pill,
    ]);

    let border_col = if is_searching {
        Color::Yellow
    } else {
        Color::Rgb(45, 75, 110)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_col));

    f.render_widget(Paragraph::new(line).block(block), area);
}

fn draw_main_body(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(66), Constraint::Percentage(34)])
        .split(area);

    draw_ports_table(f, app, chunks[0]);
    draw_inspector(f, app, chunks[1]);
}

fn draw_ports_table(f: &mut Frame, app: &mut App, area: Rect) {
    let sort_col = app.sort_by;
    let arrow = if app.sort_desc { "▼" } else { "▲" };

    let col_title = |col: SortColumn, title: &'static str| -> String {
        if sort_col == col {
            format!("{} {}", title, arrow)
        } else {
            title.to_string()
        }
    };

    let header_cells = [
        col_title(SortColumn::Port, "PORT"),
        "PROTO".to_string(),
        "SERVICE".to_string(),
        col_title(SortColumn::Pid, "PID"),
        col_title(SortColumn::Process, "PROCESS"),
        col_title(SortColumn::Cpu, "CPU%"),
        col_title(SortColumn::Memory, "RAM (MB)"),
        "STATE".to_string(),
    ]
    .into_iter()
    .map(|h| {
        Cell::from(h).style(
            Style::default()
                .fg(Color::Rgb(80, 220, 255))
                .add_modifier(Modifier::BOLD),
        )
    });

    let header = Row::new(header_cells)
        .style(Style::default().bg(Color::Rgb(20, 28, 40)))
        .height(1);

    let rows: Vec<Row> = app
        .filtered_entries
        .iter()
        .map(|entry| {
            let is_dev = entry.is_dev_port();
            let port_style = if is_dev {
                Style::default()
                    .fg(Color::Rgb(255, 210, 60))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            };

            let port_label = if is_dev {
                format!("★ :{}", entry.local_port)
            } else {
                format!("  :{}", entry.local_port)
            };

            let proto_style = match entry.protocol {
                Protocol::Tcp => Style::default().fg(Color::Cyan),
                Protocol::Udp => Style::default().fg(Color::Magenta),
            };

            let service_str = entry.service_hint.unwrap_or("-");
            let service_style = if is_dev {
                Style::default().fg(Color::Rgb(255, 200, 90))
            } else {
                Style::default().fg(Color::Gray)
            };

            let cpu_style = if entry.cpu_usage > 25.0 {
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
            } else if entry.cpu_usage > 5.0 {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::Rgb(150, 220, 170))
            };

            let mem_mb = entry.memory_mb();
            let mem_style = if mem_mb > 500.0 {
                Style::default()
                    .fg(Color::Rgb(255, 130, 130))
                    .add_modifier(Modifier::BOLD)
            } else if mem_mb > 150.0 {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::Rgb(190, 165, 255))
            };

            let state_style = if entry.is_listening {
                Style::default().fg(Color::Rgb(0, 230, 150))
            } else {
                Style::default().fg(Color::DarkGray)
            };

            Row::new(vec![
                Cell::from(port_label).style(port_style),
                Cell::from(entry.protocol.as_str()).style(proto_style),
                Cell::from(service_str).style(service_style),
                Cell::from(entry.pid.to_string()).style(Style::default().fg(Color::Gray)),
                Cell::from(entry.process_name.clone()).style(
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Cell::from(format!("{:5.1}%", entry.cpu_usage)).style(cpu_style),
                Cell::from(format!("{:6.1}", mem_mb)).style(mem_style),
                Cell::from(entry.state.clone()).style(state_style),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(10),
        Constraint::Length(6),
        Constraint::Length(15),
        Constraint::Length(8),
        Constraint::Min(16),
        Constraint::Length(8),
        Constraint::Length(9),
        Constraint::Length(12),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .title(Span::styled(
                    " ACTIVE PORTS & PROCESSES ",
                    Style::default()
                        .fg(Color::Rgb(80, 220, 255))
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Rgb(45, 75, 110))),
        )
        .row_highlight_style(
            Style::default()
                .bg(Color::Rgb(35, 58, 90))
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    f.render_stateful_widget(table, area, &mut app.table_state);
}

fn draw_inspector(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(Span::styled(
            " PROCESS INSPECTOR ",
            Style::default()
                .fg(Color::Rgb(255, 195, 70))
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(45, 75, 110)));

    let Some(entry) = app.selected_entry() else {
        let empty = Paragraph::new("No port/process selected.\nAdjust filters or press [Esc] to clear search.")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center)
            .block(block);
        f.render_widget(empty, area);
        return;
    };

    let siblings = app.sibling_ports_for_pid(entry.pid);
    let sibling_str = if siblings.is_empty() {
        "-".to_string()
    } else {
        siblings
            .iter()
            .take(10)
            .map(|p| format!(":{}", p))
            .collect::<Vec<_>>()
            .join(", ")
    };

    let run_mins = entry.run_time_secs / 60;
    let run_secs = entry.run_time_secs % 60;
    let read_mb = entry.disk_read_bytes as f64 / (1024.0 * 1024.0);
    let write_mb = entry.disk_write_bytes as f64 / (1024.0 * 1024.0);

    let lines = vec![
        Line::from(vec![
            Span::styled("Process:  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                &entry.process_name,
                Style::default()
                    .fg(Color::Rgb(80, 230, 255))
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("PID:      ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                entry.pid.to_string(),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("   "),
            Span::styled("Uptime: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{}m {:02}s", run_mins, run_secs),
                Style::default().fg(Color::Gray),
            ),
        ]),
        Line::from(vec![
            Span::styled("Port:     ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!(":{} ({})", entry.local_port, entry.protocol.as_str()),
                Style::default()
                    .fg(Color::Rgb(255, 210, 60))
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Service:  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                entry.service_hint.unwrap_or("Custom / Unknown"),
                Style::default().fg(Color::Rgb(0, 230, 150)),
            ),
        ]),
        Line::from(vec![
            Span::styled("Bind:     ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{}:{}", entry.local_addr, entry.local_port),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("Remote:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                if entry.remote_port > 0 {
                    format!("{}:{}", entry.remote_addr, entry.remote_port)
                } else {
                    "*:*".to_string()
                },
                Style::default().fg(Color::Gray),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("CPU Load: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{:.2}%", entry.cpu_usage),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::raw("   "),
            Span::styled("Memory: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{:.1} MB", entry.memory_mb()),
                Style::default()
                    .fg(Color::Rgb(190, 150, 255))
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Disk I/O: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("R: {:.1} MB | W: {:.1} MB", read_mb, write_mb),
                Style::default().fg(Color::Gray),
            ),
        ]),
        Line::from(vec![
            Span::styled("All PID Ports: ", Style::default().fg(Color::DarkGray)),
            Span::styled(sibling_str, Style::default().fg(Color::Rgb(255, 200, 100))),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Executable Path:",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            entry.exe_path.clone(),
            Style::default().fg(Color::Gray),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Command Line:",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            entry.cmd_line.clone(),
            Style::default().fg(Color::Rgb(170, 190, 210)),
        )),
        Line::from(""),
        Line::from(Span::styled(
            " [k / Del] Kill Process   [?] Help ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Rgb(255, 95, 95))
                .add_modifier(Modifier::BOLD),
        )),
    ];

    let p = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    if let Some((msg, is_ok)) = app.active_status() {
        let (bg, fg, icon) = if is_ok {
            (Color::Rgb(0, 190, 120), Color::Black, "✔")
        } else {
            (Color::Rgb(220, 60, 60), Color::White, "✖")
        };
        let line = Line::from(vec![
            Span::styled(
                format!(" {} {} ", icon, msg),
                Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                "(Press any action key to continue)",
                Style::default().fg(Color::DarkGray),
            ),
        ]);
        f.render_widget(Paragraph::new(line), area);
        return;
    }

    let keys = [
        ("↑/↓", "Navigate"),
        ("/", "Search"),
        ("k/Del", "Kill PID"),
        ("l", "Listen/All"),
        ("d", "Dev Ports"),
        ("p", "Proto"),
        ("s/r", "Sort"),
        ("Space", "Pause"),
        ("?", "Help"),
        ("q", "Quit"),
    ];

    let mut spans = Vec::new();
    for (k, desc) in keys {
        spans.push(Span::styled(
            format!(" [{}] ", k),
            Style::default()
                .fg(Color::Rgb(80, 220, 255))
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!("{} ", desc),
            Style::default().fg(Color::Gray),
        ));
    }

    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_kill_modal(f: &mut Frame, app: &App, area: Rect) {
    let Some(entry) = app.selected_entry() else {
        return;
    };

    let popup_area = centered_rect(52, 32, area);
    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(Span::styled(
            " ⚠ CONFIRM PROCESS TERMINATION ",
            Style::default()
                .fg(Color::White)
                .bg(Color::Red)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));

    let text = vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("Are you sure you want to terminate "),
            Span::styled(
                &entry.process_name,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("?"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("PID: ", Style::default().fg(Color::Gray)),
            Span::styled(
                entry.pid.to_string(),
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            ),
            Span::raw("   "),
            Span::styled("Port: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!(":{} ({})", entry.local_port, entry.protocol.as_str()),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                " [y / Enter] KILL PROCESS ",
                Style::default()
                    .fg(Color::White)
                    .bg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("   "),
            Span::styled(
                " [n / Esc] Cancel ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Gray)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
    ];

    let p = Paragraph::new(text)
        .alignment(Alignment::Center)
        .block(block)
        .wrap(Wrap { trim: true });
    f.render_widget(p, popup_area);
}

fn draw_help_modal(f: &mut Frame, area: Rect) {
    let popup_area = centered_rect(64, 65, area);
    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(Span::styled(
            " ⌨ PROCDASH KEYBOARD SHORTCUTS & HELP ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Rgb(80, 220, 255))
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(80, 220, 255)));

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "Navigation & Search",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("  ↑ / ↓ or j / k     Move selection up / down"),
        Line::from("  PgUp / PgDn        Jump 10 rows up / down"),
        Line::from("  /                  Focus live search bar (filter by port, name, PID)"),
        Line::from("  Esc                Clear search filter / close modal"),
        Line::from(""),
        Line::from(Span::styled(
            "Port & Process Actions",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("  k or Delete        Kill selected process (opens confirmation modal)"),
        Line::from("  l                  Toggle LISTEN-only vs All socket states"),
        Line::from("  d                  Toggle Developer Ports filter (3000, 5173, 8080, etc.)"),
        Line::from("  p                  Cycle Protocol filter (TCP+UDP -> TCP -> UDP)"),
        Line::from("  s or Tab           Cycle sort column (PORT -> CPU% -> RAM -> PID -> NAME)"),
        Line::from("  r                  Reverse sort order (Ascending / Descending)"),
        Line::from("  Space              Pause / Resume real-time telemetry updates"),
        Line::from("  q or Ctrl+C        Quit ProcDash"),
        Line::from(""),
        Line::from(Span::styled(
            "Press [Esc], [?], or [Enter] to close help",
            Style::default().fg(Color::Cyan),
        )),
    ];

    let p = Paragraph::new(lines).block(block).wrap(Wrap { trim: false });
    f.render_widget(p, popup_area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
