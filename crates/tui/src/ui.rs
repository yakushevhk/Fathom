use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Gauge, List, ListItem, Paragraph, Sparkline, Wrap},
    Frame,
};
use crate::app::{App, Dialog, InputMode, LogLevel, Panel};

/// Main draw function: assembles the full TUI layout.
pub fn draw(frame: &mut Frame, app: &App) {
    let size = frame.area();

    // Top-level vertical split: header | body | footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // header with progress
            Constraint::Min(10),   // body
            Constraint::Length(3), // footer / input
        ])
        .split(size);

    draw_header(frame, app, chunks[0]);
    draw_body(frame, app, chunks[1]);
    draw_footer(frame, app, chunks[2]);

    // Modal overlays on top of everything.
    if let Some(ref dialog) = app.dialog {
        match dialog {
            Dialog::Help => draw_help_overlay(frame, size),
            Dialog::SessionBrowser => draw_session_browser(frame, app, size),
            Dialog::Confirm(msg) => draw_confirm_dialog(frame, msg, size),
            Dialog::FilePicker => draw_file_picker(frame, app, size),
        }
    }
}

// ---------------------------------------------------------------------------
// Header
// ---------------------------------------------------------------------------

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let elapsed = app.elapsed();
    let minutes = elapsed.as_secs() / 60;
    let seconds = elapsed.as_secs() % 60;

    // Split header into title, token gauge and token-history sparkline.
    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(40),
            Constraint::Length(30),
            Constraint::Length(24),
        ])
        .split(area);

    let mode_tag = if app.replay_mode { " [REPLAY]" } else { "" };
    let title = if let Some(ref session_id) = app.session_id {
        format!(
            " Parallel Research{} | Session: {} | {}:{:02} ",
            mode_tag,
            &session_id.0[..8.min(session_id.0.len())],
            minutes,
            seconds
        )
    } else {
        format!(" Parallel Research{} | {}:{:02} ", mode_tag, minutes, seconds)
    };

    let header = Paragraph::new(title)
        .style(
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Left)
        .block(Block::default().borders(Borders::ALL).title(" Status "));

    frame.render_widget(header, header_chunks[0]);

    // Token usage progress bar
    let ratio = app.token_usage_ratio();
    let pct = (ratio * 100.0) as u16;
    let label = format!(
        "{}/{} ({pct}%)",
        format_tokens(app.total_tokens),
        format_tokens(app.context_window),
    );

    let gauge_color = if ratio > 0.9 {
        Color::Red
    } else if ratio > 0.75 {
        Color::Yellow
    } else {
        Color::Green
    };

    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title(" Tokens "))
        .gauge_style(Style::default().fg(gauge_color).bg(Color::DarkGray))
        .ratio(ratio)
        .label(label);

    frame.render_widget(gauge, header_chunks[1]);

    // Token-consumption sparkline over the session's lifetime.
    let spark = Sparkline::default()
        .block(Block::default().borders(Borders::ALL).title(" Usage "))
        .data(&app.token_history)
        .style(Style::default().fg(Color::Cyan));
    frame.render_widget(spark, header_chunks[2]);
}

// ---------------------------------------------------------------------------
// Body
// ---------------------------------------------------------------------------

fn draw_body(frame: &mut Frame, app: &App, area: Rect) {
    // Horizontal split: left sidebar | right content
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(70),
        ])
        .split(area);

    draw_left_panel(frame, app, chunks[0]);
    draw_right_panel(frame, app, chunks[1]);
}

// ---------------------------------------------------------------------------
// Left panel: Agents (top) + Tools (bottom)
// ---------------------------------------------------------------------------

fn draw_left_panel(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(60),
            Constraint::Percentage(40),
        ])
        .split(area);

    draw_agents_panel(frame, app, chunks[0]);
    match app.selected_panel {
        Panel::Jobs => draw_jobs_panel(frame, app, chunks[1]),
        Panel::Memory => draw_memory_panel(frame, app, chunks[1]),
        _ => draw_tools_panel(frame, app, chunks[1]),
    }
}

fn draw_agents_panel(frame: &mut Frame, app: &App, area: Rect) {
    let border_style = if app.selected_panel == Panel::Agents {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    // Summary line
    let active = app.active_agent_count();
    let completed = app.completed_agent_count();

    let mut items: Vec<ListItem> = Vec::new();

    items.push(ListItem::new(Line::from(vec![
        Span::styled(
            format!("Active: {} ", active),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("Done: {} ", completed),
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        ),
    ])));
    items.push(ListItem::new(""));

    // Tree order (DFS) honouring collapsed nodes; cursor indexes this list.
    let visible = app.visible_agents();
    let panel_active = app.selected_panel == Panel::Agents;

    for (idx, agent_id) in visible.iter().enumerate() {
        let Some(agent) = app.agents.get(agent_id) else {
            continue;
        };
        let indent = "  ".repeat(agent.depth as usize);
        // Expand/collapse marker for nodes with children.
        let branch = if app.has_children(agent_id) {
            if app.collapsed.contains(agent_id) { "▸ " } else { "▾ " }
        } else {
            "  "
        };

        let (state_icon, state_color) = match &agent.state {
            pr_core::AgentState::Idle => ("○", Color::DarkGray),
            pr_core::AgentState::Planning { .. } => ("◑", Color::Yellow),
            pr_core::AgentState::Researching { .. } => ("◐", Color::Cyan),
            pr_core::AgentState::Analyzing => ("◑", Color::Blue),
            pr_core::AgentState::Synthesizing => ("◕", Color::Magenta),
            pr_core::AgentState::Writing => ("✎", Color::White),
            pr_core::AgentState::Complete => ("✓", Color::Green),
            pr_core::AgentState::Error { .. } => ("✗", Color::Red),
        };

        let elapsed = app.agent_elapsed(&agent.id);
        let elapsed_str = format_elapsed_short(elapsed);

        // Active tool indicator
        let active_tool = app.active_tools.get(&agent.id).map(|t| t.as_str());

        let selected = panel_active && idx == app.agents_cursor;
        let row_style = if selected {
            Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        let mut spans = vec![
            Span::styled(indent.clone(), row_style),
            Span::styled(branch, Style::default().fg(Color::DarkGray)),
            Span::styled(state_icon, Style::default().fg(state_color)),
            Span::raw(" "),
            Span::styled(
                agent.role.clone(),
                Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
            ),
        ];

        // Token count
        if agent.tokens > 0 {
            spans.push(Span::styled(
                format!(" [{}tk]", agent.tokens),
                Style::default().fg(Color::DarkGray),
            ));
        }

        // Elapsed time
        spans.push(Span::styled(
            format!(" {}", elapsed_str),
            Style::default().fg(Color::DarkGray),
        ));

        items.push(ListItem::new(Line::from(spans)).style(row_style));

        // Task (truncated) — hidden for collapsed nodes to keep the tree compact.
        let task_display = agent.task.chars().take(40).collect::<String>();
        items.push(ListItem::new(Line::from(vec![
            Span::raw(format!("{}    ", indent)),
            Span::styled(task_display, Style::default().fg(Color::DarkGray)),
        ])));

        if app.collapsed.contains(agent_id) {
            continue;
        }

        // Active tool call
        if let Some(tool) = active_tool {
            items.push(ListItem::new(Line::from(vec![
                Span::raw(format!("{}    ", indent)),
                Span::styled(
                    format!("→ {}", tool),
                    Style::default().fg(Color::Blue).add_modifier(Modifier::ITALIC),
                ),
            ])));
        }

        // Last 2 tool calls
        for tc in agent.tool_calls.iter().rev().take(2) {
            items.push(ListItem::new(Line::from(vec![
                Span::raw(format!("{}    ", indent)),
                Span::styled(tc.clone(), Style::default().fg(Color::DarkGray)),
            ])));
        }
    }

    let title = format!(" Agents ({}) ", app.agents.len());
    let agents_list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(border_style),
    );

    frame.render_widget(agents_list, area);
}

fn draw_tools_panel(frame: &mut Frame, app: &App, area: Rect) {
    let border_style = Style::default().fg(Color::DarkGray);

    let mut items: Vec<ListItem> = Vec::new();

    // Show active tools at the top
    if !app.active_tools.is_empty() {
        items.push(ListItem::new(Line::from(vec![
            Span::styled(
                "Active:",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
        ])));
        for (agent_id, tool) in &app.active_tools {
            let agent_label = app
                .agents
                .get(agent_id)
                .map(|a| a.role.clone())
                .unwrap_or_else(|| agent_id.0.clone());
            items.push(ListItem::new(Line::from(vec![
                Span::styled("→ ", Style::default().fg(Color::Blue)),
                Span::styled(tool.clone(), Style::default().fg(Color::Cyan)),
                Span::styled(format!(" ({})", agent_label), Style::default().fg(Color::DarkGray)),
            ])));
        }
        items.push(ListItem::new(""));
    }

    // Recent completed tool calls (last 10)
    let recent: Vec<_> = app
        .tool_calls
        .iter()
        .rev()
        .filter(|tc| tc.duration_ms.is_some())
        .take(10)
        .collect();

    if !recent.is_empty() {
        items.push(ListItem::new(Line::from(vec![
            Span::styled(
                "Recent:",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
        ])));
        for tc in recent {
            let dur = tc.duration_ms.unwrap_or(0);
            let dur_str = if dur >= 1000 {
                format!("{:.1}s", dur as f64 / 1000.0)
            } else {
                format!("{}ms", dur)
            };
            items.push(ListItem::new(Line::from(vec![
                Span::styled("✓ ", Style::default().fg(Color::Green)),
                Span::raw(tc.tool.clone()),
                Span::styled(format!(" ({})", dur_str), Style::default().fg(Color::DarkGray)),
            ])));
        }
    }

    let title = format!(" Tools ({}) ", app.tool_calls.len());
    let tools_list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(border_style),
    );

    frame.render_widget(tools_list, area);
}

fn draw_jobs_panel(frame: &mut Frame, app: &App, area: Rect) {
    let border_style = Style::default().fg(Color::Cyan);

    let mut items: Vec<ListItem> = Vec::new();

    if app.jobs.is_empty() {
        items.push(ListItem::new(Line::from(vec![Span::styled(
            "no jobs — submit via CLI or server API",
            Style::default().fg(Color::DarkGray),
        )])));
    } else {
        for job in app.jobs.iter().take(10) {
            let stale = job.status == "running"
                && job
                    .pid
                    .map(|p| !pr_persistence::pid_alive(p))
                    .unwrap_or(false);
            let status = if stale { "stale" } else { &job.status };
            let (glyph, style) = match status {
                "queued" => ("·", Style::default().fg(Color::DarkGray)),
                "running" => ("▶", Style::default().fg(Color::Yellow)),
                "completed" => ("✓", Style::default().fg(Color::Green)),
                "failed" => ("✗", Style::default().fg(Color::Red)),
                "cancelled" => ("⊘", Style::default().fg(Color::DarkGray)),
                _ => ("?", Style::default().fg(Color::Magenta)),
            };
            let short_id = &job.id[..job.id.len().min(8)];
            let task_preview: String = job.task.chars().take(30).collect();
            items.push(ListItem::new(Line::from(vec![
                Span::styled(format!("{glyph} "), style),
                Span::styled(short_id.to_string(), Style::default().fg(Color::Blue)),
                Span::styled(format!(" {status:9}"), style),
                Span::styled(
                    format!(" {}/{}", job.attempt, job.max_attempts),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw(format!(" {task_preview}")),
            ])));
        }
        if app.jobs.len() > 10 {
            items.push(ListItem::new(Span::styled(
                format!("… {} more", app.jobs.len() - 10),
                Style::default().fg(Color::DarkGray),
            )));
        }
    }

    let title = format!(" Jobs ({}) ", app.jobs.len());
    let jobs_list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(border_style),
    );

    frame.render_widget(jobs_list, area);
}

fn draw_memory_panel(frame: &mut Frame, app: &App, area: Rect) {
    let border_style = Style::default().fg(Color::Magenta);

    let snap = &app.memory_snapshot;
    let mut items: Vec<ListItem> = Vec::new();

    if app.memory.is_none() {
        items.push(ListItem::new(Line::from(vec![Span::styled(
            "memory disabled — set [memory] enabled = true",
            Style::default().fg(Color::DarkGray),
        )])));
    } else {
        // Stats header line.
        items.push(ListItem::new(Line::from(vec![
            Span::styled(
                format!("agent:{} ", snap.agent_active),
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("user:{} ", snap.user_active),
                Style::default().fg(Color::Cyan),
            ),
            Span::styled(
                format!("run:{} ", snap.run_active),
                Style::default().fg(Color::Yellow),
            ),
            Span::styled(
                format!("graph:{}n/{}e", snap.entity_nodes, snap.entity_edges),
                Style::default().fg(Color::Blue),
            ),
        ])));
        items.push(ListItem::new(""));

        if snap.recent.is_empty() {
            items.push(ListItem::new(Line::from(vec![Span::styled(
                "no memories yet — agents store findings via memory_absorb",
                Style::default().fg(Color::DarkGray),
            )])));
        } else {
            for line in &snap.recent {
                let width = area.width.saturating_sub(16) as usize;
                let preview: String = line.content.chars().take(width.max(10)).collect();
                items.push(ListItem::new(Line::from(vec![
                    Span::styled(
                        match line.scope.as_str() {
                            "user" => "u",
                            "run" => "r",
                            _ => "a",
                        }
                        .to_string(),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(
                        format!(" {} ", line.id),
                        Style::default().fg(Color::Blue),
                    ),
                    Span::raw(preview),
                ])));
            }
        }
    }

    let total = snap.agent_active + snap.user_active + snap.run_active;
    let memory_list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" Memory ({total}) "))
            .border_style(border_style),
    );

    frame.render_widget(memory_list, area);
}

// ---------------------------------------------------------------------------
// Right panel: Output/Thinking (top) + Event Log (bottom)
// ---------------------------------------------------------------------------

fn draw_right_panel(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(60),
            Constraint::Percentage(40),
        ])
        .split(area);

    draw_output_panel(frame, app, chunks[0]);
    draw_log_panel(frame, app, chunks[1]);
}

fn draw_output_panel(frame: &mut Frame, app: &App, area: Rect) {
    let border_style = if app.selected_panel == Panel::Output {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    // If thinking is active and not collapsed, split into thinking + output
    let has_thinking = !app.should_auto_hide_thinking()
        && app.thinking.values().any(|t| !t.content.is_empty());

    if has_thinking && !app.thinking_collapsed {
        let inner_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(8),
                Constraint::Min(4),
            ])
            .split(area);

        draw_thinking_subpanel(frame, app, inner_chunks[0]);
        draw_output_subpanel(frame, app, inner_chunks[1], border_style);
    } else {
        draw_output_subpanel(frame, app, area, border_style);
    }
}

fn draw_thinking_subpanel(frame: &mut Frame, app: &App, area: Rect) {
    // Combine all thinking content
    let mut thinking_text = String::new();
    for (id, state) in &app.thinking {
        if !state.content.is_empty() {
            let agent_label = app
                .agents
                .get(id)
                .map(|a| format!("[{}] ", a.role))
                .unwrap_or_default();
            thinking_text.push_str(&agent_label);
            thinking_text.push_str(&state.content);
            thinking_text.push('\n');
        }
    }

    let visible_height = area.height.saturating_sub(2) as usize;
    let lines: Vec<&str> = thinking_text.lines().collect();
    let start = lines.len().saturating_sub(visible_height);
    let display_text: String = lines[start..].join("\n");

    let thinking_para = Paragraph::new(display_text)
        .style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC))
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Thinking (t to toggle) ")
                .border_style(Style::default().fg(Color::DarkGray))
                .style(Style::default().bg(Color::Rgb(20, 25, 35))),
        );

    frame.render_widget(thinking_para, area);
}

fn draw_output_subpanel(frame: &mut Frame, app: &App, area: Rect, border_style: Style) {
    let content = if app.output_text.is_empty() {
        // Show hint text when no output
        "Output will appear here when agents produce results.\n\n\
         Press 'i' to enter a query and start a research session."
            .to_string()
    } else {
        app.output_text.clone()
    };

    let visible_height = area.height.saturating_sub(2) as usize;
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.len().saturating_sub(visible_height);
    let display_lines: Vec<Line> = lines[start..]
        .iter()
        .map(|l| Line::from(Span::raw(l.to_string())))
        .collect();

    let output_para = Paragraph::new(display_lines)
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Output ")
                .border_style(border_style),
        );

    frame.render_widget(output_para, area);
}

fn draw_log_panel(frame: &mut Frame, app: &App, area: Rect) {
    let border_style = if app.selected_panel == Panel::Log {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let visible_height = area.height.saturating_sub(2) as usize;
    let start = app
        .event_log
        .len()
        .saturating_sub(visible_height + app.scroll_offset as usize);
    let end = app
        .event_log
        .len()
        .saturating_sub(app.scroll_offset as usize);

    let items: Vec<ListItem> = app.event_log[start..end.min(app.event_log.len())]
        .iter()
        .map(|entry| {
            let (prefix, color) = match entry.level {
                LogLevel::Info => ("\u{2022}", Color::White),
                LogLevel::Success => ("\u{2713}", Color::Green),
                LogLevel::Error => ("\u{2717}", Color::Red),
                LogLevel::Tool => ("\u{2192}", Color::Blue),
            };

            let time = entry.time.format("%H:%M:%S").to_string();
            ListItem::new(Line::from(vec![
                Span::styled(prefix, Style::default().fg(color)),
                Span::raw(" "),
                Span::styled(time, Style::default().fg(Color::DarkGray)),
                Span::raw(" "),
                Span::raw(entry.message.chars().take(80).collect::<String>()),
            ]))
        })
        .collect();

    let log_list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Event Log ")
            .border_style(border_style),
    );

    frame.render_widget(log_list, area);
}

// ---------------------------------------------------------------------------
// Footer / Input
// ---------------------------------------------------------------------------

fn draw_footer(frame: &mut Frame, app: &App, area: Rect) {
    let is_active = app.selected_panel == Panel::Input || app.input_mode == InputMode::Insert;

    let border_style = if is_active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    // Pending operator interactions override the normal hints so the user
    // immediately sees what is expected of them.
    let pending_hint = if app.pending_question.is_some() {
        Some(" ❓ AGENT QUESTION — type the answer and press Enter".to_string())
    } else if app.pending_approval.is_some() {
        Some(" 🔐 APPROVAL NEEDED — y: allow | n: deny".to_string())
    } else {
        None
    };

    let (prompt, input_text, help_text) = match app.input_mode {
        InputMode::Insert => {
            let lines: Vec<&str> = app.input.split('\n').collect();
            let current_line = lines.last().copied().unwrap_or("");
            (
                "> ",
                current_line.to_string(),
                " Enter: submit | Shift+Enter: newline | Esc: normal mode | Up/Down: history".to_string(),
            )
        }
        InputMode::Paste => (
            "Paste> ",
            app.paste_buffer.clone(),
            " Esc: end paste".to_string(),
        ),
        InputMode::Normal => (
            "",
            String::new(),
            " i: insert | q: quit | Tab: panels | ?: help | t: thinking | c: clear | b: sessions".to_string(),
        ),
    };

    let help_text = pending_hint.unwrap_or(help_text);

    let display_text = if app.input_mode == InputMode::Insert || app.input_mode == InputMode::Paste {
        format!("{}{}", prompt, input_text)
    } else {
        help_text.clone()
    };

    let style = if app.input_mode == InputMode::Insert {
        Style::default().fg(Color::White)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let title = if app.input_mode == InputMode::Insert {
        " Query Input [INSERT] "
    } else if app.input_mode == InputMode::Paste {
        " Query Input [PASTE] "
    } else {
        " Query Input "
    };

    let input = Paragraph::new(display_text)
        .style(style)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(border_style),
        );

    frame.render_widget(input, area);
}

// ---------------------------------------------------------------------------
// Help overlay
// ---------------------------------------------------------------------------

/// Modal keymap reference (`?` toggles).
fn draw_help_overlay(frame: &mut Frame, area: Rect) {
    let lines = vec![
        Line::from(Span::styled(
            "Parallel Research — keymap",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("  q / Ctrl+C      quit"),
        Line::from("  i               insert mode (type a query)"),
        Line::from("  Enter           submit query (insert mode)"),
        Line::from("  Shift+Enter     newline in input"),
        Line::from("  Esc             back to normal mode / input panel"),
        Line::from("  Tab / BackTab   cycle panels"),
        Line::from("  Up / Down       scroll (or move agent cursor in Agents panel)"),
        Line::from("  Left / Right    collapse / expand agent subtree"),
        Line::from("  t               toggle thinking panel"),
        Line::from("  c               clear output"),
        Line::from("  b               session browser"),
        Line::from("  y / n           approve / deny pending tool call"),
        Line::from("  @               file reference autocomplete (insert mode)"),
        Line::from("  Ctrl+Z          undo last file change"),
        Line::from("  Scroll          mouse wheel to scroll"),
        Line::from("  ?               this help"),
        Line::from(""),
        Line::from(Span::styled(
            "Press ? to close",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    // Center a 56x22 box.
    let w = 56.min(area.width.saturating_sub(2));
    let h = 22.min(area.height.saturating_sub(2));
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let box_area = Rect::new(x, y, w, h);

    frame.render_widget(Clear, box_area);
    let overlay = Paragraph::new(lines)
        .style(Style::default().fg(Color::White))
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title(" Help "),
        );
    frame.render_widget(overlay, box_area);
}

// ---------------------------------------------------------------------------
// Session browser dialog
// ---------------------------------------------------------------------------

fn draw_session_browser(frame: &mut Frame, app: &App, area: Rect) {
    let w = 70.min(area.width.saturating_sub(4));
    let h = 20.min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let box_area = Rect::new(x, y, w, h);

    frame.render_widget(Clear, box_area);

    let mut items: Vec<ListItem> = Vec::new();
    if app.session_list.is_empty() {
        items.push(ListItem::new(Line::from(vec![Span::styled(
            "No sessions found",
            Style::default().fg(Color::DarkGray),
        )])));
    } else {
        for (idx, session) in app.session_list.iter().take(15).enumerate() {
            let selected = idx == app.file_ref_selected;
            let status_icon = match session.status.as_str() {
                "completed" => "✓",
                "running" => "▶",
                "failed" => "✗",
                _ => "·",
            };
            let query_preview: String = session.query.chars().take(40).collect();
            let style = if selected {
                Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            items.push(ListItem::new(Line::from(vec![
                Span::styled(format!("{} ", status_icon), style),
                Span::styled(&session.id.0[..8.min(session.id.0.len())], Style::default().fg(Color::Blue)),
                Span::raw(" "),
                Span::styled(query_preview, style),
            ])).style(style));
        }
    }

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(" Sessions (↑↓: navigate, Enter: select, Esc: close) "),
    );
    frame.render_widget(list, box_area);
}

// ---------------------------------------------------------------------------
// Confirm dialog
// ---------------------------------------------------------------------------

fn draw_confirm_dialog(frame: &mut Frame, message: &str, area: Rect) {
    let w = 50.min(area.width.saturating_sub(4));
    let h = 7.min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let box_area = Rect::new(x, y, w, h);

    frame.render_widget(Clear, box_area);
    let para = Paragraph::new(vec![
        Line::from(Span::raw(message)),
        Line::from(""),
        Line::from(Span::styled(
            "Press any key to continue",
            Style::default().fg(Color::DarkGray),
        )),
    ])
    .wrap(Wrap { trim: false })
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .title(" Confirm "),
    );
    frame.render_widget(para, box_area);
}

// ---------------------------------------------------------------------------
// File picker dialog
// ---------------------------------------------------------------------------

fn draw_file_picker(frame: &mut Frame, app: &App, area: Rect) {
    let w = 50.min(area.width.saturating_sub(4));
    let h = 15.min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let box_area = Rect::new(x, y, w, h);

    frame.render_widget(Clear, box_area);

    let mut items: Vec<ListItem> = Vec::new();
    items.push(ListItem::new(Line::from(vec![
        Span::styled(format!("@{} ", app.file_ref_query), Style::default().fg(Color::Yellow)),
    ])));
    items.push(ListItem::new(""));

    if app.file_refs.is_empty() {
        items.push(ListItem::new(Span::styled(
            "No matching files",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for (idx, file) in app.file_refs.iter().take(10).enumerate() {
            let selected = idx == app.file_ref_selected;
            let style = if selected {
                Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let icon = if file.ends_with('/') { "📁" } else { "📄" };
            items.push(ListItem::new(Line::from(vec![
                Span::styled(format!("{} ", icon), style),
                Span::styled(file.clone(), style),
            ])).style(style));
        }
    }

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .title(" File Reference (Tab: accept, Esc: cancel) "),
    );
    frame.render_widget(list, box_area);
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Format token count in human-readable form (e.g. "12.5k", "1.2M")
fn format_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        format!("{}", n)
    }
}

/// Format a duration as a short string (e.g. "1.2s", "3m 12s")
fn format_elapsed_short(d: std::time::Duration) -> String {
    let secs = d.as_secs();
    if secs >= 60 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else if secs >= 1 {
        format!("{}.{:.0}s", secs, d.subsec_millis() / 100)
    } else {
        format!("{}ms", d.subsec_millis())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_tokens_small() {
        assert_eq!(format_tokens(0), "0");
        assert_eq!(format_tokens(42), "42");
        assert_eq!(format_tokens(999), "999");
    }

    #[test]
    fn test_format_tokens_thousands() {
        assert_eq!(format_tokens(1_000), "1.0k");
        assert_eq!(format_tokens(1_500), "1.5k");
        assert_eq!(format_tokens(12_345), "12.3k");
    }

    #[test]
    fn test_format_tokens_millions() {
        assert_eq!(format_tokens(1_000_000), "1.0M");
        assert_eq!(format_tokens(2_500_000), "2.5M");
    }

    #[test]
    fn test_format_elapsed_millis() {
        let d = std::time::Duration::from_millis(500);
        assert_eq!(format_elapsed_short(d), "500ms");
    }

    #[test]
    fn test_format_elapsed_seconds() {
        let d = std::time::Duration::from_millis(2500);
        assert_eq!(format_elapsed_short(d), "2.5s");
    }

    #[test]
    fn test_format_elapsed_minutes() {
        let d = std::time::Duration::from_secs(195);
        assert_eq!(format_elapsed_short(d), "3m 15s");
    }
}
