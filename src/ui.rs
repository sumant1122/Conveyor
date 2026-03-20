use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap, Table, Row, Tabs, Cell},
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
        vec![" [1] Dashboard ", " [2] Pipeline Config ", " [3] Env Variables "]
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
    let area = frame.area();
    
    // RESPONSIVE: If height is too small, skip header/footer to save space
    let show_header = area.height > 10;
    let show_footer = area.height > 15;

    let vertical_constraints = if show_header && show_footer {
        vec![Constraint::Length(3), Constraint::Min(0), Constraint::Length(1)]
    } else if show_header {
        vec![Constraint::Length(3), Constraint::Min(0)]
    } else {
        vec![Constraint::Min(0)]
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vertical_constraints)
        .split(area);

    let mut current_chunk = 0;

    if show_header {
        draw_header(frame, chunks[current_chunk], pipeline_name, git_info, current_view);
        current_chunk += 1;
    }

    match current_view {
        AppView::Dashboard => draw_dashboard(frame, chunks[current_chunk], states, selected_job, log_scroll),
        AppView::Settings => draw_settings(frame, chunks[current_chunk], pipeline),
        AppView::EnvVars => draw_env_vars(frame, chunks[current_chunk], user_env),
    }

    if show_footer {
        draw_footer(frame, chunks[chunks.len() - 1], states);
    }
}

fn draw_header(frame: &mut Frame, area: Rect, pipeline_name: &str, git_info: &str, current_view: &AppView) {
    // FLEXIBLE HEADER: Don't use fixed percentages that crush text
    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(30),   // Pipeline & Tabs get priority
            Constraint::Max(40),   // Git info stays on the right
        ])
        .split(area);

    let tabs = Tabs::new(AppView::titles())
        .block(Block::default().borders(Borders::ALL).title(format!(" {} ", pipeline_name)))
        .highlight_style(Style::default().fg(Color::Yellow).bold())
        .select(current_view.to_index());
    frame.render_widget(tabs, header_chunks[0]);

    if header_chunks[1].width > 10 {
        let git_p = Paragraph::new(format!(" {} ", git_info))
            .block(Block::default().borders(Borders::ALL).title(" Git "))
            .style(Style::default().fg(Color::Cyan));
        frame.render_widget(git_p, header_chunks[1]);
    }
}

fn draw_dashboard(frame: &mut Frame, area: Rect, states: &[JobState], selected_job: usize, log_scroll: u16) {
    // FLEXIBLE DASHBOARD: Ensure logs get at least 60% or remaining space
    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(30), // Fixed width for jobs list
            Constraint::Min(0),     // Logs take all the rest
        ])
        .split(area);

    // Job List
    let items: Vec<ListItem> = states
        .iter()
        .enumerate()
        .map(|(i, state)| {
            let (marker, color) = match state.status {
                JobStatus::Pending => (" o ", Color::Gray),
                JobStatus::Running => (" > ", Color::Yellow),
                JobStatus::Success => (" v ", Color::Green),
                JobStatus::Failed => (" x ", Color::Red),
            };

            let mut style = Style::default().fg(color);
            if i == selected_job {
                style = style.add_modifier(Modifier::REVERSED);
            }

            let line = Line::from(vec![
                Span::styled(marker, style),
                Span::styled(state.name.clone(), style),
            ]);

            ListItem::new(line)
        })
        .collect();

    let job_list = List::new(items)
        .block(Block::default().title(" Jobs ").borders(Borders::ALL));
    frame.render_widget(job_list, content_chunks[0]);

    // Logs
    let (logs, line_count) = if let Some(state) = states.get(selected_job) {
        if state.logs.is_empty() {
            ("No logs yet...".to_string(), 0)
        } else {
            (state.logs.join("\n"), state.logs.len())
        }
    } else {
        ("No job selected".to_string(), 0)
    };

    let title = if let Some(state) = states.get(selected_job) {
        format!(" Logs: {} [{} lines] ", state.name, line_count)
    } else {
        " Logs ".to_string()
    };

    let log_view = Paragraph::new(logs)
        .block(Block::default().title(title).borders(Borders::ALL))
        .wrap(Wrap { trim: true })
        .scroll((log_scroll, 0));
    frame.render_widget(log_view, content_chunks[1]);
}

fn draw_settings(frame: &mut Frame, area: Rect, pipeline: &Pipeline) {
    let mut rows = Vec::new();

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
    }

    rows.push(Row::new(vec![Cell::from(""); 3]));

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
    .block(Block::default().title(" Pipeline Configuration ").borders(Borders::ALL));

    frame.render_widget(table, area);
}

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
    .block(Block::default().title(" Local Environment (env.yaml) ").borders(Borders::ALL));

    frame.render_widget(table, area);
}
