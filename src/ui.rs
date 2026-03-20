use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap, Table, Row},
    Frame,
};
use crate::runner::{JobState, JobStatus};
use crate::pipeline::Pipeline;

#[derive(PartialEq)]
pub enum AppView {
    Dashboard,
    Settings,
}

pub fn draw(
    frame: &mut Frame,
    states: &[JobState],
    selected_job: usize,
    git_info: &str,
    pipeline_name: &str,
    current_view: &AppView,
    pipeline: &Pipeline,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Content
        ])
        .split(frame.area());

    // Header
    let running_count = states.iter().filter(|s| s.status == JobStatus::Running).count();
    let view_name = match current_view {
        AppView::Dashboard => "Dashboard",
        AppView::Settings => "Settings",
    };
    let header_text = format!(" {} | {} | {} | Running: {}", pipeline_name, git_info, view_name, running_count);
    let header = Paragraph::new(header_text)
        .block(Block::default().borders(Borders::ALL).title("Conveyor Dashboard [1: Dash, 2: Settings]"))
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
    frame.render_widget(header, chunks[0]);

    match current_view {
        AppView::Dashboard => draw_dashboard(frame, chunks[1], states, selected_job),
        AppView::Settings => draw_settings(frame, chunks[1], pipeline),
    }
}

fn draw_dashboard(frame: &mut Frame, area: ratatui::layout::Rect, states: &[JobState], selected_job: usize) {
    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(area);

    // Job List
    let items: Vec<ListItem> = states
        .iter()
        .enumerate()
        .map(|(i, state)| {
            let status_symbol = match state.status {
                JobStatus::Pending => "○",
                JobStatus::Running => "▶",
                JobStatus::Success => "✔",
                JobStatus::Failed => "✘",
            };
            let color = match state.status {
                JobStatus::Pending => Color::Gray,
                JobStatus::Running => Color::Yellow,
                JobStatus::Success => Color::Green,
                JobStatus::Failed => Color::Red,
            };

            let mut style = Style::default().fg(color);
            if i == selected_job {
                style = style.add_modifier(Modifier::BOLD).bg(Color::DarkGray);
            }

            ListItem::new(format!("{} {}", status_symbol, state.name)).style(style)
        })
        .collect();

    let job_list = List::new(items)
        .block(Block::default().title("Jobs").borders(Borders::ALL))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(job_list, content_chunks[0]);

    // Logs
    let logs = if let Some(state) = states.get(selected_job) {
        state.logs.join("\n")
    } else {
        "No job selected".to_string()
    };

    let log_view = Paragraph::new(logs)
        .block(Block::default().title("Logs").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    frame.render_widget(log_view, content_chunks[1]);
}

fn draw_settings(frame: &mut Frame, area: ratatui::layout::Rect, pipeline: &Pipeline) {
    let mut rows = Vec::new();

    // Global Env
    rows.push(Row::new(vec!["Global".to_string(), "".to_string(), "".to_string()]).style(Style::default().add_modifier(Modifier::BOLD).fg(Color::Yellow)));
    if let Some(env) = &pipeline.env {
        for (k, v) in env {
            rows.push(Row::new(vec!["".to_string(), k.clone(), v.clone()]));
        }
    } else {
        rows.push(Row::new(vec!["".to_string(), "None".to_string(), "".to_string()]));
    }

    // Job Envs
    for job in &pipeline.jobs {
        rows.push(Row::new(vec![format!("Job: {}", job.name), "".to_string(), "".to_string()]).style(Style::default().add_modifier(Modifier::BOLD).fg(Color::Yellow)));
        if let Some(env) = &job.env {
            for (k, v) in env {
                rows.push(Row::new(vec!["".to_string(), k.clone(), v.clone()]));
            }
        } else {
            rows.push(Row::new(vec!["".to_string(), "None".to_string(), "".to_string()]));
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
            .style(Style::default().add_modifier(Modifier::BOLD))
            .bottom_margin(1),
    )
    .block(Block::default().title("Environment Variables").borders(Borders::ALL));

    frame.render_widget(table, area);
}
