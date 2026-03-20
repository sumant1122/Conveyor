use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap, Table, Row, Tabs, BorderType},
    Frame,
};
use crate::runner::{JobState, JobStatus};
use crate::pipeline::Pipeline;

#[derive(PartialEq, Clone, Copy)]
pub enum AppView {
    Dashboard,
    Settings,
    EnvVars,
}

impl AppView {
    pub fn to_index(&self) -> usize {
        match self {
            AppView::Dashboard => 0,
            AppView::Settings => 1,
            AppView::EnvVars => 2,
        }
    }

    pub fn titles() -> Vec<&'static str> {
        vec!["[1] Dashboard", "[2] Pipeline Config", "[3] Env Variables"]
    }
}

pub fn draw(
    frame: &mut Frame,
    states: &[JobState],
    selected_job: usize,
    git_info: &str,
    pipeline_name: &str,
    current_view: &AppView,
    pipeline: &Pipeline,
    user_env: &std::collections::HashMap<String, String>,
    log_scroll: u16,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header/Tabs
            Constraint::Min(0),    // Content
            Constraint::Length(1), // Footer
        ])
        .split(frame.area());

    draw_header(frame, chunks[0], pipeline_name, git_info, current_view);

    match current_view {
        AppView::Dashboard => draw_dashboard(frame, chunks[1], states, selected_job, log_scroll),
        AppView::Settings => draw_settings(frame, chunks[1], pipeline),
        AppView::EnvVars => draw_env_vars(frame, chunks[1], user_env),
    }

    draw_footer(frame, chunks[2], states);
}

fn draw_header(frame: &mut Frame, area: Rect, pipeline_name: &str, git_info: &str, current_view: &AppView) {
    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(area);

    let tabs = Tabs::new(AppView::titles())
        .block(Block::default().borders(Borders::ALL).title(format!(" Conveyor: {} ", pipeline_name)).border_type(BorderType::Rounded))
        .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
        .select(current_view.to_index());
    frame.render_widget(tabs, header_chunks[0]);

    let git_p = Paragraph::new(format!(" {} ", git_info))
        .block(Block::default().borders(Borders::ALL).title(" Git context ").border_type(BorderType::Rounded))
        .style(Style::default().fg(Color::Cyan));
    frame.render_widget(git_p, header_chunks[1]);
}

fn draw_footer(frame: &mut Frame, area: Rect, states: &[JobState]) {
    let total = states.len();
    let success = states.iter().filter(|s| s.status == JobStatus::Success).count();
    let failed = states.iter().filter(|s| s.status == JobStatus::Failed).count();
    let running = states.iter().filter(|s| s.status == JobStatus::Running).count();

    let text = format!(
        " [q] Quit | [↑/↓] Job | [PgUp/PgDn/Home/End] Scroll Logs | Status: {} Total, {} Success, {} Failed, {} Running ",
        total, success, failed, running
    );
    
    let footer = Paragraph::new(text)
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));
    frame.render_widget(footer, area);
}

fn draw_dashboard(frame: &mut Frame, area: Rect, states: &[JobState], selected_job: usize, log_scroll: u16) {
    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(area);

    // Job List with Timeline
    let items: Vec<ListItem> = states
        .iter()
        .enumerate()
        .map(|(i, state)| {
            let (marker, color, tag) = match state.status {
                JobStatus::Pending => (" ○ ", Color::Gray, "WAIT"),
                JobStatus::Running => (" ▶ ", Color::Yellow, "RUN "),
                JobStatus::Success => (" ✔ ", Color::Green, "DONE"),
                JobStatus::Failed => (" ✘ ", Color::Red, "FAIL"),
            };

            let mut style = Style::default().fg(color);
            let is_selected = i == selected_job;
            
            if is_selected {
                style = style.add_modifier(Modifier::BOLD).bg(Color::Rgb(40, 44, 52));
            }

            let connector = if i == 0 { "  " } else { "│ " };
            let connector_style = Style::default().fg(Color::DarkGray);

            let line = Line::from(vec![
                Span::styled(connector, connector_style),
                Span::styled("──", connector_style),
                Span::styled(marker, style),
                Span::styled("── ", connector_style),
                Span::styled(format!("{:<12}", state.name), style),
                Span::styled(format!(" [{}]", tag), style.add_modifier(Modifier::DIM)),
            ]);

            ListItem::new(line)
        })
        .collect();

    let job_list = List::new(items)
        .block(Block::default()
            .title(" Pipeline Timeline ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::DarkGray)))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(job_list, content_chunks[0]);

    // Logs
    let logs = if let Some(state) = states.get(selected_job) {
        if state.logs.is_empty() {
            "No logs yet...".to_string()
        } else {
            state.logs.join("\n")
        }
    } else {
        "No job selected".to_string()
    };

    let title = if let Some(state) = states.get(selected_job) {
        format!(" Logs: {} ", state.name)
    } else {
        " Logs ".to_string()
    };

    let log_view = Paragraph::new(logs)
        .block(Block::default().title(title).borders(Borders::ALL).border_type(BorderType::Rounded))
        .wrap(Wrap { trim: true })
        .scroll((log_scroll, 0));
    frame.render_widget(log_view, content_chunks[1]);
}

fn draw_settings(frame: &mut Frame, area: Rect, pipeline: &Pipeline) {
    let mut rows = Vec::new();

    // Global Env
    rows.push(Row::new(vec![
        Cell::from("GLOBAL").bold().yellow(),
        Cell::from(""),
        Cell::from(""),
    ]));

    if let Some(env) = &pipeline.env {
        for (k, v) in env {
            rows.push(Row::new(vec![
                Cell::from(""),
                Cell::from(k.clone()),
                Cell::from(v.clone()).italic().dim(),
            ]));
        }
    } else {
        rows.push(Row::new(vec![
            Cell::from(""),
            Cell::from("None").gray(),
            Cell::from(""),
        ]));
    }

    rows.push(Row::new(vec![Cell::from(""); 3])); // Spacer

    // Job Envs
    for job in &pipeline.jobs {
        rows.push(Row::new(vec![
            Cell::from(format!("JOB: {}", job.name)).bold().yellow(),
            Cell::from(""),
            Cell::from(""),
        ]));
        if let Some(env) = &job.env {
            for (k, v) in env {
                rows.push(Row::new(vec![
                    Cell::from(""),
                    Cell::from(k.clone()),
                    Cell::from(v.clone()).italic().dim(),
                ]));
            }
        } else {
            rows.push(Row::new(vec![
                Cell::from(""),
                Cell::from("None").gray(),
                Cell::from(""),
            ]));
        }
    }

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(30),
            Constraint::Percentage(30),
            Constraint::Percentage(40),
        ],
    )
    .header(
        Row::new(vec!["Scope", "Key", "Value"])
            .style(Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan))
            .bottom_margin(1),
    )
    .block(Block::default().title(" Pipeline Configuration ").borders(Borders::ALL).border_type(BorderType::Rounded));

    frame.render_widget(table, area);
}

use ratatui::widgets::Cell;

fn draw_env_vars(frame: &mut Frame, area: Rect, env: &std::collections::HashMap<String, String>) {
    let mut rows = Vec::new();
    for (k, v) in env {
        rows.push(Row::new(vec![
            Cell::from(k.clone()).bold().cyan(),
            Cell::from(v.clone()).italic(),
        ]));
    }

    if rows.is_empty() {
        rows.push(Row::new(vec![
            Cell::from("No environment variables found in env.yaml").gray(),
            Cell::from(""),
        ]));
    }

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(40),
            Constraint::Percentage(60),
        ],
    )
    .header(
        Row::new(vec!["Variable Name", "Value"])
            .style(Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan))
            .bottom_margin(1),
    )
    .block(Block::default().title(" Local Environment (env.yaml) ").borders(Borders::ALL).border_type(BorderType::Rounded));

    frame.render_widget(table, area);
}
