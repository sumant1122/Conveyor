use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};
use crate::runner::{JobState, JobStatus};

pub fn draw(frame: &mut Frame, states: &[JobState], selected_job: usize, git_info: &str, pipeline_name: &str) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Content
        ])
        .split(frame.area());

    // Header
    let running_count = states.iter().filter(|s| s.status == JobStatus::Running).count();
    let header_text = format!(" {} | {} | Running: {}", pipeline_name, git_info, running_count);
    let header = Paragraph::new(header_text)
        .block(Block::default().borders(Borders::ALL).title("Conveyor Dashboard"))
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
    frame.render_widget(header, chunks[0]);

    // Content
    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(chunks[1]);

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
