use orrery_core::checkpoint::StepStatus;
use orrery_core::config::{AgentBackend, ExecutionMode, HookEvent};
use orrery_core::stream_log::{StreamEvent, StreamEventKind};
use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};

use crate::app::{App, Focus, Modal, RunStatus, Selector, SelectorKind};
use crate::layout::{BANNER_MIN_HEIGHT, StageColumn, UiLayoutRegions, UiRegion};
use crate::theme::{
    AMBER_ALERT, AMBER_BRIGHT, AMBER_DARK, AMBER_DIM, AMBER_PRIMARY, BANNER, BG_BLACK,
    COMPACT_HEADER, PANEL_BG, alert_block, banner_line, base_block, focused_block, highlight,
    muted,
};

const TOO_SMALL_NOTICE: &str = "Terminal too small (need 80×24)";

pub fn render(frame: &mut Frame, app: &App, regions: &UiLayoutRegions) {
    let area = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().fg(AMBER_PRIMARY).bg(BG_BLACK)),
        area,
    );

    if regions.too_small {
        render_too_small(frame, area);
        return;
    }

    render_header(frame, app, regions.header, area.height >= BANNER_MIN_HEIGHT);
    render_top_menu(frame, app, regions);
    render_steps_panel(frame, app, regions.steps_panel);
    render_stage_canvas(frame, app, regions);
    render_live_stream(frame, app, regions.live_stream);
    render_footer_menu(frame, app, regions);
    render_modal(frame, app, area);
}

fn inner_rect(rect: Rect) -> Rect {
    if rect.width < 2 || rect.height < 2 {
        return Rect::default();
    }
    Rect::new(rect.x + 1, rect.y + 1, rect.width - 2, rect.height - 2)
}

fn panel_block(focused: bool, title: &'static str) -> Block<'static> {
    if focused {
        focused_block().title(title)
    } else {
        base_block().title(title)
    }
}

fn panel_bg(focused: bool) -> Color {
    if focused { PANEL_BG } else { BG_BLACK }
}

fn render_too_small(frame: &mut Frame, area: Rect) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let y = area.y + area.height / 2;
    let row = Rect::new(area.x, y, area.width, 1);
    let notice = Paragraph::new(TOO_SMALL_NOTICE)
        .alignment(Alignment::Center)
        .style(Style::default().fg(AMBER_ALERT).bg(BG_BLACK));
    frame.render_widget(notice, row);
}

fn status_badge_label(status: RunStatus) -> &'static str {
    match status {
        RunStatus::Idle => "[IDLE]",
        RunStatus::Running => "[RUNNING]",
        RunStatus::AwaitingOperator => "[AWAITING OPERATOR]",
        RunStatus::Passed => "[PASSED]",
        RunStatus::Error => "[ERROR]",
    }
}

fn status_badge_style(status: RunStatus) -> Style {
    match status {
        RunStatus::Idle => muted(),
        RunStatus::Running => highlight(),
        RunStatus::AwaitingOperator => Style::default()
            .fg(AMBER_ALERT)
            .bg(BG_BLACK)
            .add_modifier(Modifier::BOLD),
        RunStatus::Passed => Style::default()
            .fg(AMBER_PRIMARY)
            .bg(BG_BLACK)
            .add_modifier(Modifier::BOLD),
        RunStatus::Error => Style::default()
            .fg(AMBER_ALERT)
            .bg(BG_BLACK)
            .add_modifier(Modifier::BOLD),
    }
}

fn render_header(frame: &mut Frame, app: &App, rect: Rect, full_banner: bool) {
    let badge = status_badge_label(app.status);
    let badge_style = status_badge_style(app.status);
    let path_display = format!(" {} ", app.project_root.display());

    let mut block = base_block()
        .title(Line::from(Span::styled(path_display, muted())))
        .title_top(Line::from(Span::styled(format!(" {badge} "), badge_style)).right_aligned());

    if let Some(msg) = &app.status_message {
        block = block.title_bottom(Line::from(Span::styled(
            format!(" {msg} "),
            Style::default().fg(AMBER_ALERT).bg(BG_BLACK),
        )));
    }

    frame.render_widget(block, rect);

    let inner = inner_rect(rect);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    if full_banner && inner.height >= 9 {
        let lines: Vec<Line<'static>> = (0..BANNER.len()).map(banner_line).collect();
        let paragraph =
            Paragraph::new(lines).style(Style::default().fg(AMBER_PRIMARY).bg(BG_BLACK));
        frame.render_widget(paragraph, inner);
    } else {
        let left_len = COMPACT_HEADER.len();
        let badge_len = badge.len();
        let inner_w = usize::from(inner.width);
        let pad = inner_w.saturating_sub(left_len + badge_len);
        let padding = " ".repeat(pad);
        let line = Line::from(vec![
            Span::styled(
                COMPACT_HEADER,
                Style::default().fg(AMBER_PRIMARY).bg(BG_BLACK),
            ),
            Span::styled(padding, Style::default().fg(AMBER_DIM).bg(BG_BLACK)),
            Span::styled(badge, badge_style),
        ]);
        let row = Rect::new(inner.x, inner.y, inner.width, 1);
        frame.render_widget(
            Paragraph::new(line).style(Style::default().fg(AMBER_PRIMARY).bg(BG_BLACK)),
            row,
        );
    }
}

fn render_top_menu(frame: &mut Frame, app: &App, regions: &UiLayoutRegions) {
    let focused = app.focus == Focus::TopMenu;
    let bg = panel_bg(focused);
    let block = panel_block(focused, " Controls ");
    frame.render_widget(block, regions.top_menu);

    let passed_count = app
        .step_statuses
        .iter()
        .filter(|&&s| s == StepStatus::Passed)
        .count();
    let total_steps = app
        .plan
        .as_ref()
        .map_or(app.step_statuses.len(), |p| p.steps.len());

    for (idx, &(region, rect)) in regions.top_items.iter().enumerate() {
        let selected = focused && app.top_index == idx;
        let (label, base_style) = match region {
            UiRegion::IngestButton => (
                "[Ingest]".to_string(),
                Style::default().fg(AMBER_PRIMARY).bg(bg),
            ),
            UiRegion::ModeToggle => {
                let mode_str = match app.config.execution_mode {
                    ExecutionMode::Run => "[Mode: RUN]",
                    ExecutionMode::Loop => "[Mode: LOOP]",
                };
                (
                    mode_str.to_string(),
                    Style::default().fg(AMBER_PRIMARY).bg(bg),
                )
            }
            UiRegion::CyclesControl => {
                let style = if app.config.execution_mode == ExecutionMode::Run {
                    muted().bg(bg)
                } else {
                    Style::default().fg(AMBER_PRIMARY).bg(bg)
                };
                (format!("[Cycles: {}]", app.config.cycle_count), style)
            }
            UiRegion::PassedCounter => (
                format!("[Passed: {passed_count}/{total_steps}]"),
                muted().bg(bg),
            ),
            UiRegion::StagesMinus => ("[-]".to_string(), Style::default().fg(AMBER_PRIMARY).bg(bg)),
            UiRegion::StagesPlus => ("[+]".to_string(), Style::default().fg(AMBER_PRIMARY).bg(bg)),
            _ => continue,
        };

        let style = if selected {
            highlight().bg(bg)
        } else {
            base_style
        };
        frame.render_widget(Paragraph::new(label).style(style), rect);

        if region == UiRegion::PassedCounter {
            let stages_label = "[Stages:";
            let stages_x = rect.x.saturating_add(rect.width).saturating_add(1);
            let right_bound = regions
                .top_menu
                .x
                .saturating_add(regions.top_menu.width)
                .saturating_sub(1);
            if stages_x < right_bound {
                let w = u16::try_from(stages_label.len())
                    .unwrap_or(8)
                    .min(right_bound - stages_x);
                frame.render_widget(
                    Paragraph::new(stages_label).style(Style::default().fg(AMBER_PRIMARY).bg(bg)),
                    Rect::new(stages_x, rect.y, w, 1),
                );
            }
        } else if region == UiRegion::StagesMinus {
            let count_str = format!(" {:>2} ", app.config.stages.len());
            let count_x = rect.x.saturating_add(rect.width).saturating_add(1);
            let right_bound = regions
                .top_menu
                .x
                .saturating_add(regions.top_menu.width)
                .saturating_sub(1);
            if count_x < right_bound {
                let w = u16::try_from(count_str.len())
                    .unwrap_or(4)
                    .min(right_bound - count_x);
                frame.render_widget(
                    Paragraph::new(count_str).style(Style::default().fg(AMBER_PRIMARY).bg(bg)),
                    Rect::new(count_x, rect.y, w, 1),
                );
            }
        } else if region == UiRegion::StagesPlus {
            let close_x = rect.x.saturating_add(rect.width).saturating_add(1);
            let right_bound = regions
                .top_menu
                .x
                .saturating_add(regions.top_menu.width)
                .saturating_sub(1);
            if close_x < right_bound {
                frame.render_widget(
                    Paragraph::new("]").style(Style::default().fg(AMBER_PRIMARY).bg(bg)),
                    Rect::new(close_x, rect.y, 1, 1),
                );
            }
        }
    }
}

fn step_status_badge(status: StepStatus) -> &'static str {
    match status {
        StepStatus::Pending => "[ ]",
        StepStatus::InProgress => "[>]",
        StepStatus::Passed => "[✓]",
        StepStatus::AwaitingOperator => "[?]",
        StepStatus::Failed => "[!]",
    }
}

fn step_status_style(status: StepStatus, bg: Color) -> Style {
    match status {
        StepStatus::Pending => muted().bg(bg),
        StepStatus::InProgress => Style::default()
            .fg(AMBER_BRIGHT)
            .bg(bg)
            .add_modifier(Modifier::BOLD),
        StepStatus::Passed => Style::default().fg(AMBER_PRIMARY).bg(bg),
        StepStatus::AwaitingOperator | StepStatus::Failed => Style::default()
            .fg(AMBER_ALERT)
            .bg(bg)
            .add_modifier(Modifier::BOLD),
    }
}

fn render_steps_panel(frame: &mut Frame, app: &App, rect: Rect) {
    let focused = app.focus == Focus::StepsPanel;
    let bg = panel_bg(focused);
    frame.render_widget(panel_block(focused, " Plan Steps "), rect);

    let inner = inner_rect(rect);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let step_count = app
        .plan
        .as_ref()
        .map_or(0, |p| p.steps.len())
        .max(app.step_statuses.len());

    if step_count == 0 {
        frame.render_widget(Paragraph::new("(no plan)").style(muted().bg(bg)), inner);
        return;
    }

    let visible_rows = usize::from(inner.height);
    let scroll_start = app.step_scroll.min(step_count);

    let mut lines = Vec::with_capacity(visible_rows);
    for idx in scroll_start..(scroll_start + visible_rows).min(step_count) {
        let status = app
            .step_statuses
            .get(idx)
            .copied()
            .unwrap_or(StepStatus::Pending);
        let badge = step_status_badge(status);
        let label = if let Some(step) = app.plan.as_ref().and_then(|p| p.steps.get(idx)) {
            format!("{badge} {}", step.title)
        } else {
            format!("{badge} Step {}", idx + 1)
        };
        let style = if focused && idx == app.step_selected {
            highlight().bg(bg)
        } else {
            step_status_style(status, bg)
        };
        lines.push(Line::from(Span::styled(label, style)));
    }

    frame.render_widget(
        Paragraph::new(lines).style(Style::default().fg(AMBER_PRIMARY).bg(bg)),
        inner,
    );
}

fn agent_backend_tag(backend: &AgentBackend) -> &'static str {
    match backend {
        AgentBackend::LocalModel { .. } => "LM",
        AgentBackend::AgentCli { .. } => "CLI",
        AgentBackend::CloudApi { .. } => "API",
    }
}

fn render_stage_canvas(frame: &mut Frame, app: &App, regions: &UiLayoutRegions) {
    let focused = app.focus == Focus::StageCanvas;
    let bg = panel_bg(focused);
    frame.render_widget(panel_block(focused, " Stages "), regions.stage_canvas);

    for col in &regions.stage_columns {
        render_stage_column(frame, app, col, focused, bg);
    }
}

fn render_stage_column(
    frame: &mut Frame,
    app: &App,
    col: &StageColumn,
    canvas_focused: bool,
    bg: Color,
) {
    let stage_selected = canvas_focused && app.stage_selected == col.index;
    let header_selected = stage_selected && app.slot_selected == 0;

    let role_label = app
        .config
        .stages
        .get(col.index)
        .map(|s| s.role_id.as_str())
        .filter(|id| !id.trim().is_empty())
        .unwrap_or("(select role)");

    let header_title = format!(" S{} ", col.index + 1);
    let header_block = if header_selected {
        focused_block().title(header_title)
    } else {
        base_block()
            .title(header_title)
            .style(Style::default().fg(AMBER_PRIMARY).bg(bg))
            .border_style(Style::default().fg(AMBER_DIM).bg(bg))
    };
    let header_bg = if header_selected { PANEL_BG } else { bg };
    let header_style = if header_selected {
        highlight().bg(header_bg)
    } else {
        Style::default().fg(AMBER_PRIMARY).bg(header_bg)
    };
    let header_widget = Paragraph::new(role_label)
        .block(header_block)
        .style(header_style);
    frame.render_widget(header_widget, col.header);

    let unlocked = app.unlocked_slots(col.index);
    let assigned = app
        .config
        .stages
        .get(col.index)
        .map_or(&[][..], |s| s.agent_ids.as_slice());

    for &(slot_idx, slot_rect) in &col.slots {
        let slot_selected = stage_selected && app.slot_selected == slot_idx + 1;
        if slot_idx >= unlocked {
            let locked_style = Style::default().fg(AMBER_DARK).bg(bg);
            frame.render_widget(Paragraph::new(" [locked]").style(locked_style), slot_rect);
            continue;
        }

        let (text, base_style) = if let Some(agent_id) = assigned.get(slot_idx) {
            let tag = app
                .agents
                .iter()
                .find(|a| a.agent_id == *agent_id)
                .map_or("?", |a| agent_backend_tag(&a.backend));
            (
                format!(" [{tag}] {agent_id}"),
                Style::default().fg(AMBER_PRIMARY).bg(bg),
            )
        } else {
            (" [+ assign]".to_string(), muted().bg(bg))
        };

        let style = if slot_selected {
            highlight().bg(bg)
        } else {
            base_style
        };
        frame.render_widget(Paragraph::new(text).style(style), slot_rect);
    }
}

fn is_alert_event(event: &StreamEvent) -> bool {
    if event.kind == StreamEventKind::Error {
        return true;
    }
    let lower = event.text.to_ascii_lowercase();
    lower.contains("denied")
        || lower.contains("denial")
        || (event.kind == StreamEventKind::GateResult && lower.contains("failed"))
}

fn stream_event_style(event: &StreamEvent, bg: Color) -> Style {
    if is_alert_event(event) {
        Style::default().fg(AMBER_ALERT).bg(bg)
    } else {
        match event.kind {
            StreamEventKind::Reasoning => muted().bg(bg),
            StreamEventKind::SystemNotice => Style::default().fg(AMBER_BRIGHT).bg(bg),
            StreamEventKind::Output
            | StreamEventKind::ToolCall
            | StreamEventKind::ReportSaved
            | StreamEventKind::GateResult => Style::default().fg(AMBER_PRIMARY).bg(bg),
            StreamEventKind::Error => Style::default().fg(AMBER_ALERT).bg(bg),
        }
    }
}

fn format_stream_event(event: &StreamEvent) -> String {
    match (&event.agent_id, event.stage_index) {
        (Some(agent), Some(stage)) => format!("[S{}:{agent}] {}", stage + 1, event.text),
        (Some(agent), None) => format!("[{agent}] {}", event.text),
        (None, _) => event.text.clone(),
    }
}

fn render_live_stream(frame: &mut Frame, app: &App, rect: Rect) {
    let focused = app.focus == Focus::LiveStream;
    let bg = panel_bg(focused);
    frame.render_widget(panel_block(focused, " Live Stream "), rect);

    let inner = inner_rect(rect);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let visible_rows = usize::from(inner.height);
    let total = app.stream.len();
    let max_scroll = total.saturating_sub(visible_rows);
    let scroll = app.stream_scroll.min(max_scroll);
    let end = total.saturating_sub(scroll);
    let start = end.saturating_sub(visible_rows);

    let mut lines = Vec::with_capacity(end - start);
    for idx in start..end {
        if let Some(event) = app.stream.get(idx) {
            let style = stream_event_style(event, bg);
            lines.push(Line::from(Span::styled(format_stream_event(event), style)));
        }
    }

    frame.render_widget(
        Paragraph::new(lines).style(Style::default().fg(AMBER_PRIMARY).bg(bg)),
        inner,
    );
}

fn render_footer_menu(frame: &mut Frame, app: &App, regions: &UiLayoutRegions) {
    let focused = app.focus == Focus::FooterMenu;
    let bg = panel_bg(focused);
    frame.render_widget(panel_block(focused, " Actions "), regions.footer_menu);

    let is_running = matches!(app.status, RunStatus::Running | RunStatus::AwaitingOperator);
    let approve_visible = app.approve_visible();

    for (idx, &(region, rect)) in regions.footer_items.iter().enumerate() {
        let label = match region {
            UiRegion::RunStopButton => {
                if is_running {
                    "[Stop]"
                } else {
                    "[Run]"
                }
            }
            UiRegion::ApproveButton => {
                if !approve_visible {
                    continue;
                }
                "[Approve]"
            }
            UiRegion::RejectButton => {
                if !approve_visible {
                    continue;
                }
                "[Reject]"
            }
            UiRegion::SettingsButton => "[Settings]",
            UiRegion::HelpButton => "[Help]",
            UiRegion::MouseToggle => {
                if app.mouse_capture {
                    "[Mouse: ON]"
                } else {
                    "[Mouse: OFF]"
                }
            }
            UiRegion::QuitButton => "[Quit]",
            _ => continue,
        };

        let selected = focused && app.footer_index == idx;
        let style = if selected {
            highlight().bg(bg)
        } else {
            Style::default().fg(AMBER_PRIMARY).bg(bg)
        };
        frame.render_widget(Paragraph::new(label).style(style), rect);
    }
}

fn centered_modal_rect(area: Rect) -> Rect {
    let w_pct = u16::try_from((u32::from(area.width) * 60) / 100).unwrap_or(40);
    let h_pct = u16::try_from((u32::from(area.height) * 50) / 100).unwrap_or(8);
    let w = w_pct.max(40).min(area.width);
    let h = h_pct.max(8).min(area.height);
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    Rect::new(x, y, w, h)
}

fn render_modal(frame: &mut Frame, app: &App, area: Rect) {
    if app.modal == Modal::None {
        return;
    }

    let modal_rect = centered_modal_rect(area);
    frame.render_widget(Clear, modal_rect);

    match &app.modal {
        Modal::None => {}
        Modal::Ingest { input } => {
            let block = focused_block().title(" Ingest Plan ");
            let lines = vec![
                Line::from(Span::styled(
                    "Enter plan file path (Enter to ingest, Esc to cancel):",
                    Style::default().fg(AMBER_DIM).bg(PANEL_BG),
                )),
                Line::from(Span::styled(
                    format!("> {input}_"),
                    Style::default()
                        .fg(AMBER_BRIGHT)
                        .bg(PANEL_BG)
                        .add_modifier(Modifier::BOLD),
                )),
            ];
            let widget = Paragraph::new(lines)
                .block(block)
                .style(Style::default().fg(AMBER_PRIMARY).bg(PANEL_BG));
            frame.render_widget(widget, modal_rect);
        }
        Modal::Settings => {
            render_settings_modal(frame, app, modal_rect);
        }
        Modal::Help => {
            render_help_modal(frame, modal_rect);
        }
        Modal::QuitConfirm => {
            let block = alert_block().title(" Confirm Quit ");
            let lines = vec![
                Line::from(Span::styled(
                    "Workflow is currently running.",
                    Style::default().fg(AMBER_ALERT).bg(PANEL_BG),
                )),
                Line::from(Span::styled(
                    "Interrupt and quit? [y/Enter = Yes, n/Esc = No]",
                    Style::default()
                        .fg(AMBER_BRIGHT)
                        .bg(PANEL_BG)
                        .add_modifier(Modifier::BOLD),
                )),
            ];
            let widget = Paragraph::new(lines)
                .block(block)
                .style(Style::default().fg(AMBER_ALERT).bg(PANEL_BG));
            frame.render_widget(widget, modal_rect);
        }
        Modal::RejectFeedback { input } => {
            let block = alert_block().title(" Reject Step ");
            let lines = vec![
                Line::from(Span::styled(
                    "Enter rejection feedback (Enter to submit, Esc to cancel):",
                    Style::default().fg(AMBER_ALERT).bg(PANEL_BG),
                )),
                Line::from(Span::styled(
                    format!("> {input}_"),
                    Style::default()
                        .fg(AMBER_BRIGHT)
                        .bg(PANEL_BG)
                        .add_modifier(Modifier::BOLD),
                )),
            ];
            let widget = Paragraph::new(lines)
                .block(block)
                .style(Style::default().fg(AMBER_ALERT).bg(PANEL_BG));
            frame.render_widget(widget, modal_rect);
        }
        Modal::Selector(selector) => {
            render_selector_modal(frame, selector, modal_rect);
        }
    }
}

fn render_settings_modal(frame: &mut Frame, app: &App, modal_rect: Rect) {
    let block = focused_block().title(" Settings (read-only — edit via CLI) ");
    let primary = Style::default().fg(AMBER_PRIMARY).bg(PANEL_BG);
    let dim = Style::default().fg(AMBER_DIM).bg(PANEL_BG);

    let role_list: String = if app.roles.is_empty() {
        "(none)".to_string()
    } else {
        app.roles
            .iter()
            .map(|r| format!("{} ({:?})", r.role_id, r.kind))
            .collect::<Vec<_>>()
            .join(", ")
    };

    let agent_list: String = if app.agents.is_empty() {
        "(none)".to_string()
    } else {
        app.agents
            .iter()
            .map(|a| format!("{} [{}]", a.agent_id, agent_backend_tag(&a.backend)))
            .collect::<Vec<_>>()
            .join(", ")
    };

    let mcp_list: String = if app.mcp_servers.is_empty() {
        "(none)".to_string()
    } else {
        app.mcp_servers
            .iter()
            .map(|s| s.server_id.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };

    let gates = if app.config.gate_tools.is_empty() {
        "(none)".to_string()
    } else {
        app.config.gate_tools.join(", ")
    };

    let mut hook_parts = Vec::new();
    for (name, ev) in [
        ("step_start", HookEvent::StepStart),
        ("step_pass", HookEvent::StepPass),
        ("step_retry", HookEvent::StepRetry),
        ("halt", HookEvent::Halt),
    ] {
        if let Some(h) = app.config.hooks.get(ev) {
            hook_parts.push(format!("{name}={}", h.tool));
        }
    }
    let hooks_str = if hook_parts.is_empty() {
        "(none)".to_string()
    } else {
        hook_parts.join(", ")
    };

    let lines = vec![
        Line::from(Span::styled(format!("Roles: {role_list}"), primary)),
        Line::from(Span::styled(format!("Agents: {agent_list}"), primary)),
        Line::from(Span::styled(format!("MCP servers: {mcp_list}"), primary)),
        Line::from(Span::styled(format!("Gate tools: {gates}"), primary)),
        Line::from(Span::styled(format!("Hooks: {hooks_str}"), primary)),
        Line::from(Span::styled("Press Esc, Enter, or q to close", dim)),
    ];

    let widget = Paragraph::new(lines).block(block).style(primary);
    frame.render_widget(widget, modal_rect);
}

fn render_help_modal(frame: &mut Frame, modal_rect: Rect) {
    let block = focused_block().title(" Help ");
    let primary = Style::default().fg(AMBER_PRIMARY).bg(PANEL_BG);
    let dim = Style::default().fg(AMBER_DIM).bg(PANEL_BG);
    let lines = vec![
        Line::from(Span::styled("Tab / Shift+Tab : Cycle panel focus", primary)),
        Line::from(Span::styled("Arrows          : Navigate / scroll", primary)),
        Line::from(Span::styled("Enter / Space   : Activate / select", primary)),
        Line::from(Span::styled("+ / -           : Adjust count", primary)),
        Line::from(Span::styled("Delete          : Unassign agent", primary)),
        Line::from(Span::styled(
            "m               : Toggle mouse capture",
            primary,
        )),
        Line::from(Span::styled("?               : Open help", primary)),
        Line::from(Span::styled("q               : Quit", primary)),
        Line::from(Span::styled("Ctrl+C          : Interrupt run", primary)),
        Line::from(Span::styled("Esc             : Close modal", dim)),
    ];
    let widget = Paragraph::new(lines).block(block).style(primary);
    frame.render_widget(widget, modal_rect);
}

fn render_selector_modal(frame: &mut Frame, selector: &Selector, modal_rect: Rect) {
    let title = match selector.kind {
        SelectorKind::Role => format!(" Select Role (Stage {}) ", selector.stage + 1),
        SelectorKind::Agent => format!(
            " Select Agent (Stage {}, Slot {}) ",
            selector.stage + 1,
            selector.slot + 1
        ),
    };
    let block = focused_block().title(title);
    let inner_h = usize::from(modal_rect.height.saturating_sub(2)).max(1);
    let scroll_start = if selector.index >= inner_h {
        selector.index + 1 - inner_h
    } else {
        0
    };

    let mut lines = Vec::new();
    for (idx, opt) in selector
        .options
        .iter()
        .enumerate()
        .skip(scroll_start)
        .take(inner_h)
    {
        if idx == selector.index {
            lines.push(Line::from(Span::styled(
                format!("> {opt}"),
                Style::default()
                    .fg(AMBER_BRIGHT)
                    .bg(PANEL_BG)
                    .add_modifier(Modifier::BOLD),
            )));
        } else {
            lines.push(Line::from(Span::styled(
                format!("  {opt}"),
                Style::default().fg(AMBER_PRIMARY).bg(PANEL_BG),
            )));
        }
    }

    let widget = Paragraph::new(lines)
        .block(block)
        .style(Style::default().fg(AMBER_PRIMARY).bg(PANEL_BG));
    frame.render_widget(widget, modal_rect);
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use orrery_core::config::{
        AgentConfig, HookConfig, HookFailure, McpServerConfig, McpTransportConfig, ProjectConfig,
        RoleConfig, RoleKind, ValidationMode, WorkspaceMount,
    };
    use orrery_core::plan::ImplementationPlan;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;

    use super::*;

    fn test_app() -> App {
        let config = ProjectConfig::default_template("demo");
        let roles = vec![
            RoleConfig {
                role_id: "orchestrator".into(),
                display_name: "Orchestrator".into(),
                kind: RoleKind::Orchestrator,
                rules_file: "roles/orchestrator.md".into(),
                allowed_mcp_tools: Vec::new(),
            },
            RoleConfig {
                role_id: "coder".into(),
                display_name: "Coder".into(),
                kind: RoleKind::Implementer,
                rules_file: "roles/coder.md".into(),
                allowed_mcp_tools: Vec::new(),
            },
            RoleConfig {
                role_id: "validator".into(),
                display_name: "Validator".into(),
                kind: RoleKind::Validator,
                rules_file: "roles/validator.md".into(),
                allowed_mcp_tools: Vec::new(),
            },
        ];
        let agents = vec![
            AgentConfig {
                agent_id: "local-qwen".into(),
                display_name: "Local Qwen".into(),
                backend: AgentBackend::LocalModel {
                    inference_id: "vllm".into(),
                    model: "qwen".into(),
                    temperature: 0.1,
                    max_context_tokens: 8192,
                },
            },
            AgentConfig {
                agent_id: "cli-worker".into(),
                display_name: "CLI Worker".into(),
                backend: AgentBackend::AgentCli {
                    image: "worker@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
                    command: vec!["worker".into()],
                    workspace_mount: WorkspaceMount::ReadWrite,
                    memory_limit_mb: 512,
                    timeout_secs: 60,
                    keychain_service: None,
                    keychain_account: None,
                    secret_env: None,
                },
            },
            AgentConfig {
                agent_id: "cloud-pro".into(),
                display_name: "Cloud Pro".into(),
                backend: AgentBackend::CloudApi {
                    endpoint_url: "https://api.example.com/v1".into(),
                    model: "pro".into(),
                    temperature: 0.0,
                    max_context_tokens: 16384,
                    keychain_service: "orrery".into(),
                    keychain_account: "testuser".into(),
                },
            },
        ];
        App::new(PathBuf::from("/workspace/demo"), config, roles, agents)
    }

    fn draw_app(app: &App, width: u16, height: u16) -> (Buffer, UiLayoutRegions) {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        let area = Rect::new(0, 0, width, height);
        let regions = UiLayoutRegions::compute(
            area,
            app.config.stages.len(),
            app.stage_scroll,
            app.slot_scroll,
        );
        terminal
            .draw(|frame| render(frame, app, &regions))
            .expect("draw frame");
        (terminal.backend().buffer().clone(), regions)
    }

    fn rect_text(buf: &Buffer, rect: Rect) -> String {
        let mut out = String::new();
        for y in rect.y..rect.y.saturating_add(rect.height) {
            for x in rect.x..rect.x.saturating_add(rect.width) {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    fn buffer_text(buf: &Buffer) -> String {
        rect_text(buf, *buf.area())
    }

    #[test]
    fn banner_at_80x40_compact_at_80x24() {
        let app = test_app();

        let (buf40, regions40) = draw_app(&app, 80, 40);
        let header40 = rect_text(&buf40, regions40.header);
        assert!(
            header40.contains(BANNER[2].trim()),
            "expected banner row 3 at 80x40, got:\n{header40}"
        );

        let (buf24, regions24) = draw_app(&app, 80, 24);
        let header24 = rect_text(&buf24, regions24.header);
        assert!(
            header24.contains(COMPACT_HEADER),
            "expected COMPACT_HEADER at 80x24, got:\n{header24}"
        );
        assert!(!header24.contains(BANNER[2].trim()));
    }

    #[test]
    fn too_small_notice_at_79x24() {
        let app = test_app();
        let (buf, regions) = draw_app(&app, 79, 24);
        assert!(regions.too_small);
        let text = buffer_text(&buf);
        assert!(text.contains("Terminal too small (need 80×24)"));
        assert!(!text.contains("[Ingest]"));
        assert!(!text.contains(COMPACT_HEADER));

        let (buf_h, regions_h) = draw_app(&app, 80, 23);
        assert!(regions_h.too_small);
        assert!(buffer_text(&buf_h).contains("Terminal too small (need 80×24)"));
        assert_eq!(inner_rect(Rect::new(0, 0, 1, 1)), Rect::default());
    }

    #[test]
    fn status_badges() {
        let mut app = test_app();
        let cases = [
            (RunStatus::Idle, "IDLE"),
            (RunStatus::Running, "RUNNING"),
            (RunStatus::AwaitingOperator, "AWAITING OPERATOR"),
            (RunStatus::Passed, "PASSED"),
            (RunStatus::Error, "ERROR"),
        ];
        for (status, expected) in cases {
            app.status = status;
            let (buf24, regions24) = draw_app(&app, 80, 24);
            let header24 = rect_text(&buf24, regions24.header);
            assert!(
                header24.contains(expected),
                "missing badge {expected} at 80x24 in:\n{header24}"
            );

            let (buf40, regions40) = draw_app(&app, 80, 40);
            let header40 = rect_text(&buf40, regions40.header);
            assert!(
                header40.contains(expected),
                "missing badge {expected} at 80x40 in:\n{header40}"
            );
        }
    }

    #[test]
    fn top_menu_labels() {
        let mut app = test_app();
        let (buf, regions) = draw_app(&app, 80, 24);
        let top_text = rect_text(&buf, regions.top_menu);
        assert!(top_text.contains("[Ingest]"), "top_menu:\n{top_text}");
        assert!(top_text.contains("[Mode: RUN]"), "top_menu:\n{top_text}");
        assert!(top_text.contains("[Stages:"), "top_menu:\n{top_text}");

        app.config.execution_mode = ExecutionMode::Loop;
        app.config.cycle_count = 3;
        let (buf_loop, regions_loop) = draw_app(&app, 80, 24);
        let loop_text = rect_text(&buf_loop, regions_loop.top_menu);
        assert!(loop_text.contains("[Mode: LOOP]"));
        assert!(loop_text.contains("[Cycles: 3]"));
    }

    #[test]
    fn steps_panel_badges() {
        let mut app = test_app();
        let md = "## Step 1: A\nbody\n## Step 2: B\nbody\n## Step 3: C\nbody\n## Step 4: D\nbody\n## Step 5: E\nbody\n";
        app.plan = Some(ImplementationPlan::parse(md, "## Step ").expect("parse plan"));
        app.step_statuses = vec![
            StepStatus::Pending,
            StepStatus::InProgress,
            StepStatus::Passed,
            StepStatus::AwaitingOperator,
            StepStatus::Failed,
        ];

        let (buf, regions) = draw_app(&app, 100, 30);
        let steps_text = rect_text(&buf, regions.steps_panel);
        for badge in ["[ ]", "[>]", "[✓]", "[?]", "[!]"] {
            assert!(
                steps_text.contains(badge),
                "missing step badge {badge} in:\n{steps_text}"
            );
        }
        assert!(
            steps_text.contains("[ ] 1: A"),
            "expected '[ ] 1: A' in:\n{steps_text}"
        );
        assert!(
            !steps_text.contains("1. 1:"),
            "unexpected '1. 1:' in:\n{steps_text}"
        );

        // Fallback step labels without parsed plan and scrolled selection when focused.
        app.plan = None;
        app.step_statuses = vec![StepStatus::Pending; 20];
        app.focus = Focus::StepsPanel;
        app.step_selected = 18;
        app.step_scroll = 15;
        let (buf_scroll, regions_scroll) = draw_app(&app, 80, 24);
        let scrolled_text = rect_text(&buf_scroll, regions_scroll.steps_panel);
        assert!(scrolled_text.contains("Step 19"));
    }

    #[test]
    fn slot_backend_tags() {
        let mut app = test_app();
        app.config.stages[1].agent_ids =
            vec!["local-qwen".into(), "cli-worker".into(), "cloud-pro".into()];
        app.focus = Focus::StageCanvas;
        app.stage_selected = 1;
        app.slot_selected = 1;

        let (buf, regions) = draw_app(&app, 100, 30);
        let canvas_text = rect_text(&buf, regions.stage_canvas);
        for tag in ["LM", "CLI", "API"] {
            assert!(
                canvas_text.contains(tag),
                "missing backend tag {tag} in:\n{canvas_text}"
            );
        }

        app.config.stages[1].role_id.clear();
        let (buf_empty_role, regions_empty_role) = draw_app(&app, 100, 30);
        let empty_role_text = rect_text(&buf_empty_role, regions_empty_role.stage_canvas);
        assert!(empty_role_text.contains("(select role)"));
    }

    #[test]
    fn locked_slots_use_amber_dark() {
        let app = test_app();
        let (buf, regions) = draw_app(&app, 100, 30);
        // Stage 0 is orchestrator (1 unlocked slot -> slot_idx >= 1 is locked).
        let col0 = &regions.stage_columns[0];
        let &(slot_idx, locked_rect) = col0
            .slots
            .iter()
            .find(|&&(idx, _)| idx >= 1)
            .expect("locked slot in stage 0");
        assert!(slot_idx >= app.unlocked_slots(0));
        for y in locked_rect.y..locked_rect.y + locked_rect.height {
            for x in locked_rect.x..locked_rect.x + locked_rect.width {
                assert_eq!(
                    buf[(x, y)].fg,
                    AMBER_DARK,
                    "locked slot cell ({x}, {y}) should have fg AMBER_DARK"
                );
            }
        }
    }

    #[test]
    fn denial_text_uses_amber_alert() {
        let mut app = test_app();
        app.push_stream_event(StreamEvent {
            step_number: Some(1),
            stage_index: Some(1),
            agent_id: Some("cli-worker".into()),
            kind: StreamEventKind::ToolCall,
            text: "filesystem:write_file denied: tool not in allowlist".into(),
            ..StreamEvent::notice("")
        });
        app.push_stream_event(StreamEvent {
            step_number: Some(1),
            stage_index: None,
            agent_id: None,
            kind: StreamEventKind::GateResult,
            text: "gate 'ci:test' failed: exit 1".into(),
            ..StreamEvent::notice("")
        });
        app.push_stream_event(StreamEvent {
            step_number: Some(1),
            stage_index: None,
            agent_id: None,
            kind: StreamEventKind::Output,
            text: "normal output".into(),
            ..StreamEvent::notice("")
        });

        let (buf, regions) = draw_app(&app, 100, 30);
        let inner = inner_rect(regions.live_stream);
        let mut found_denial_cell = false;
        for y in inner.y..inner.y + inner.height {
            let row_str: String = (inner.x..inner.x + inner.width)
                .map(|x| buf[(x, y)].symbol())
                .collect();
            if row_str.contains("denied") {
                found_denial_cell = true;
                for x in inner.x..inner.x + inner.width {
                    if buf[(x, y)].symbol() != " " {
                        assert_eq!(buf[(x, y)].fg, AMBER_ALERT);
                    }
                }
            }
        }
        assert!(found_denial_cell, "expected denial line in live_stream");
    }

    #[test]
    fn approve_reject_only_when_awaiting_gate_and_operator() {
        let mut app = test_app();
        app.config.validation_mode = ValidationMode::GateAndOperator;
        app.awaiting_operator = false;
        app.mouse_capture = false;

        let (buf_idle, regions_idle) = draw_app(&app, 100, 30);
        let footer_idle = rect_text(&buf_idle, regions_idle.footer_menu);
        assert!(!footer_idle.contains("[Approve]"));
        assert!(!footer_idle.contains("[Reject]"));
        assert!(footer_idle.contains("[Mouse: OFF]"));

        app.awaiting_operator = true;
        app.status = RunStatus::AwaitingOperator;
        app.config.validation_mode = ValidationMode::Agents;
        let (buf_agents, regions_agents) = draw_app(&app, 100, 30);
        let footer_agents = rect_text(&buf_agents, regions_agents.footer_menu);
        assert!(!footer_agents.contains("[Approve]"));
        assert!(!footer_agents.contains("[Reject]"));

        app.config.validation_mode = ValidationMode::GateAndOperator;
        let (buf_gate, regions_gate) = draw_app(&app, 100, 30);
        let footer_gate = rect_text(&buf_gate, regions_gate.footer_menu);
        assert!(footer_gate.contains("[Approve]"));
        assert!(footer_gate.contains("[Reject]"));
    }

    #[test]
    fn every_cell_has_black_or_panel_background() {
        let mut app = test_app();
        app.status_message = Some("validation warning".into());
        app.config.hooks.set(
            HookEvent::StepPass,
            HookConfig {
                tool: "git:commit".into(),
                on_failure: HookFailure::Warn,
            },
        );
        app.config.gate_tools = vec!["ci:test".into()];
        app.push_stream_event(StreamEvent::notice("workflow ready"));
        app.push_stream_event(StreamEvent {
            step_number: Some(1),
            stage_index: None,
            agent_id: Some("local-qwen".into()),
            kind: StreamEventKind::Reasoning,
            text: "thinking".into(),
            ..StreamEvent::notice("")
        });
        app.push_stream_event(StreamEvent {
            step_number: Some(1),
            stage_index: None,
            agent_id: None,
            kind: StreamEventKind::Error,
            text: "error line".into(),
            ..StreamEvent::notice("")
        });

        let modals = [
            Modal::None,
            Modal::Ingest {
                input: "STEPPED_IMPLEMENTATION_PLAN.md".into(),
            },
            Modal::Settings,
            Modal::Help,
            Modal::QuitConfirm,
            Modal::RejectFeedback {
                input: "fix tests".into(),
            },
            Modal::Selector(Selector {
                stage: 1,
                slot: 0,
                kind: SelectorKind::Agent,
                options: vec!["local-qwen".into(), "(remove)".into()],
                index: 0,
            }),
            Modal::Selector(Selector {
                stage: 0,
                slot: 0,
                kind: SelectorKind::Role,
                options: (0..15).map(|i| format!("role-{i}")).collect(),
                index: 14,
            }),
        ];

        for focus in [
            Focus::TopMenu,
            Focus::StepsPanel,
            Focus::StageCanvas,
            Focus::LiveStream,
            Focus::FooterMenu,
        ] {
            app.focus = focus;
            for modal in &modals {
                app.modal = modal.clone();
                for (w, h) in [(79, 24), (80, 24), (120, 40)] {
                    let (buf, _) = draw_app(&app, w, h);
                    for y in 0..h {
                        for x in 0..w {
                            let bg = buf[(x, y)].bg;
                            assert!(
                                bg == BG_BLACK || bg == PANEL_BG,
                                "cell ({x}, {y}) at {w}x{h} with focus={focus:?} modal={modal:?} had bg {bg:?}"
                            );
                        }
                    }
                }
            }
        }

        // Also check Settings modal when roles, agents, gates, and hooks are empty.
        app.roles.clear();
        app.agents.clear();
        app.config.gate_tools.clear();
        app.config.hooks.clear(HookEvent::StepPass);
        app.modal = Modal::Settings;
        let (buf_empty, _) = draw_app(&app, 80, 24);
        assert!(buffer_text(&buf_empty).contains("(none)"));
    }

    #[test]
    fn settings_lists_mcp_servers() {
        let mut app = test_app();
        app.mcp_servers = vec![McpServerConfig {
            server_id: "fs".into(),
            transport: McpTransportConfig::Stdio {
                command: "mcp-fs".into(),
                args: Vec::new(),
            },
            write_tools: Vec::new(),
        }];
        app.modal = Modal::Settings;
        let (buf, _) = draw_app(&app, 100, 30);
        let text = buffer_text(&buf);
        assert!(
            text.contains("MCP servers: fs"),
            "expected 'MCP servers: fs' in Settings modal, got:\n{text}"
        );
    }
}
