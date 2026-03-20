use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, List, ListItem, Paragraph, Wrap, Table, Row, Tabs, Cell, Padding},
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

    pub fn titles() -> Vec<Line<'static>> {
        vec![
            Line::from(vec![" 󰄬 ".cyan(), "Dashboard ".into()]),
            Line::from(vec![" 󰘦 ".magenta(), "Pipeline ".into()]),
            Line::from(vec![" 󱐋 ".yellow(), "Environment ".into()]),
        ]
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
    
    // RESPONSIVE: If height is too small, skip header/footer
    let show_header = area.height > 6;
    let show_footer = area.height > 12;

    let constraints = match (show_header, show_footer) {
        (true, true) => vec![Constraint::Length(3), Constraint::Min(0), Constraint::Length(1)],
        (true, false) => vec![Constraint::Length(3), Constraint::Min(0)],
        (false, true) => vec![Constraint::Min(0), Constraint::Length(1)],
        (false, false) => vec![Constraint::Min(0)],
    };

    let chunks = Layout::vertical(constraints).split(area);
    let mut current_chunk = 0;

    if show_header {
        draw_header(frame, chunks[current_chunk], pipeline_name, git_info, current_view);
        current_chunk += 1;
    }

    let main_area = chunks[current_chunk];
    match current_view {
        AppView::Dashboard => draw_dashboard(frame, main_area, states, selected_job, log_scroll),
        AppView::Settings => draw_settings(frame, main_area, pipeline),
        AppView::EnvVars => draw_env_vars(frame, main_area, user_env),
    }

    if show_footer {
        draw_footer(frame, chunks[chunks.len() - 1], states);
    }
}

fn draw_header(frame: &mut Frame, area: Rect, pipeline_name: &str, git_info: &str, current_view: &AppView) {
    let chunks = Layout::horizontal([
        Constraint::Min(40),
        Constraint::Max(40),
    ]).split(area);

    let tabs = Tabs::new(AppView::titles())
        .block(Block::bordered().title(Line::from(vec![" ".into(), pipeline_name.bold().cyan(), " ".into()])))
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .select(current_view.to_index())
        .padding(" ", " ");
    
    frame.render_widget(tabs, chunks[0]);

    if chunks[1].width > 15 {
        let git_p = Paragraph::new(Line::from(vec![
            " 󰊢 ".cyan().bold(),
            git_info.dim().into(),
        ]))
        .block(Block::bordered().title(" Git Context "))
        .alignment(ratatui::layout::Alignment::Right);
        frame.render_widget(git_p, chunks[1]);
    }
}

fn draw_dashboard(frame: &mut Frame, area: Rect, states: &[JobState], selected_job: usize, log_scroll: u16) {
    let chunks = Layout::horizontal([
        Constraint::Length(30),
        Constraint::Min(0),
    ]).split(area);

    // Job List with better styling
    let items: Vec<ListItem> = states
        .iter()
        .enumerate()
        .map(|(i, state)| {
            let (icon, color) = match state.status {
                JobStatus::Pending => ("󰑐 ", Color::Gray),
                JobStatus::Running => ("󱑮 ", Color::Yellow),
                JobStatus::Success => ("󰄬 ", Color::Green),
                JobStatus::Failed => ("󰅖 ", Color::Red),
            };

            let mut style = Style::default().fg(color);
            if i == selected_job {
                style = style.bg(Color::Rgb(40, 44, 52)).add_modifier(Modifier::BOLD);
            }

            let line = Line::from(vec![
                Span::styled(format!(" {} ", icon), style),
                Span::styled(state.name.clone(), style),
            ]);

            ListItem::new(line)
        })
        .collect();

    let job_list = List::new(items)
        .block(Block::bordered().title(" Pipeline Jobs ").padding(Padding::horizontal(1)))
        .highlight_symbol(">> ")
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));
    
    frame.render_widget(job_list, chunks[0]);

    // Logs with better UI
    let (logs, line_count) = if let Some(state) = states.get(selected_job) {
        if state.logs.is_empty() {
            ("No logs generated yet for this job.".dim().to_string(), 0)
        } else {
            (state.logs.join("\n"), state.logs.len())
        }
    } else {
        ("Select a job to view logs.".italic().to_string(), 0)
    };

    let job_name = states.get(selected_job).map(|s| s.name.as_str()).unwrap_or("None");
    let title = Line::from(vec![
        " Logs: ".into(),
        job_name.bold().yellow(),
        format!(" [{} lines] ", line_count).dim(),
    ]);

    let log_view = Paragraph::new(logs)
        .block(Block::bordered().title(title).padding(Padding::uniform(1)))
        .wrap(Wrap { trim: true })
        .scroll((log_scroll, 0));
    
    frame.render_widget(log_view, chunks[1]);
}

fn draw_settings(frame: &mut Frame, area: Rect, pipeline: &Pipeline) {
    let mut rows = Vec::new();

    rows.push(Row::new(vec![
        Cell::from("GLOBAL").bold().cyan(),
        Cell::from(""),
        Cell::from(""),
    ]));

    if let Some(env) = &pipeline.env {
        for (k, v) in env {
            rows.push(Row::new(vec![
                Cell::from("  ├─").dim(),
                Cell::from(k.clone()).yellow(),
                Cell::from(v.clone()).italic().dim(),
            ]));
        }
    }

    rows.push(Row::new(vec![Cell::from(""); 3]));

    for job in &pipeline.jobs {
        rows.push(Row::new(vec![
            Cell::from(format!("JOB: {}", job.name)).bold().magenta(),
            Cell::from(""),
            Cell::from(""),
        ]));
        if let Some(env) = &job.env {
            for (k, v) in env {
                rows.push(Row::new(vec![
                    Cell::from("  ├─").dim(),
                    Cell::from(k.clone()).yellow(),
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
        Row::new(vec!["Scope", "Variable", "Value"])
            .style(Style::default().bold().underlined().fg(Color::Cyan))
            .bottom_margin(1),
    )
    .block(Block::bordered().title(" Pipeline Configuration ").padding(Padding::uniform(1)));

    frame.render_widget(table, area);
}

fn draw_env_vars(frame: &mut Frame, area: Rect, env: &std::collections::HashMap<String, String>) {
    let mut rows = Vec::new();
    let mut keys: Vec<_> = env.keys().collect();
    keys.sort();

    for k in keys {
        rows.push(Row::new(vec![
            Cell::from(k.clone()).bold().yellow(),
            Cell::from(env.get(k).unwrap().clone()).italic(),
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
            .style(Style::default().bold().underlined().fg(Color::Cyan))
            .bottom_margin(1),
    )
    .block(Block::bordered().title(" Local Environment (env.yaml) ").padding(Padding::uniform(1)));

    frame.render_widget(table, area);
}

fn draw_footer(frame: &mut Frame, area: Rect, states: &[JobState]) {
    let total = states.len();
    let success = states.iter().filter(|s| s.status == JobStatus::Success).count();
    let failed = states.iter().filter(|s| s.status == JobStatus::Failed).count();
    let running = states.iter().filter(|s| s.status == JobStatus::Running).count();

    let status_line = Line::from(vec![
        " [q] Quit ".bold().red(),
        "│".dim(),
        " [1-3] Views ".bold().blue(),
        "│".dim(),
        " [↑↓] Jobs ".bold().yellow(),
        "│".dim(),
        " [PgUp/Dn] Logs ".bold().magenta(),
        "│ ".dim(),
        format!(" {} Total ", total).into(),
        "│ ".dim(),
        format!(" {} OK ", success).green(),
        "│ ".dim(),
        format!(" {} Fail ", failed).red(),
        "│ ".dim(),
        format!(" {} Run ", running).yellow(),
    ]);
    
    let footer = Paragraph::new(status_line)
        .bg(Color::Rgb(30, 30, 30));
    frame.render_widget(footer, area);
}
