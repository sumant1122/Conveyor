use ratatui::{
    layout::{Constraint, Layout, Rect, Alignment},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, List, ListItem, Paragraph, Wrap, Table, Row, Tabs, Cell, Padding, Borders},
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
            Line::from(vec![" [D] ".cyan(), "Dashboard".into()]),
            Line::from(vec![" [P] ".magenta(), "Pipeline".into()]),
            Line::from(vec![" [E] ".yellow(), "Environment".into()]),
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
    search_query: &str,
) {
    let area = frame.area();
    
    let constraints = vec![
        Constraint::Length(1), // Header
        Constraint::Min(0),    // Main Content
        Constraint::Length(1), // Footer
    ];

    let chunks = Layout::vertical(constraints).split(area);

    draw_header(frame, chunks[0], pipeline_name, git_info, current_view);
    
    let main_area = chunks[1];
    match current_view {
        AppView::Dashboard => draw_dashboard(frame, main_area, states, selected_job, log_scroll, search_query),
        AppView::Settings => draw_settings(frame, main_area, pipeline),
        AppView::EnvVars => draw_env_vars(frame, main_area, user_env),
    }

    draw_footer(frame, chunks[2], states, search_query);
}

fn draw_header(frame: &mut Frame, area: Rect, pipeline_name: &str, git_info: &str, current_view: &AppView) {
    let chunks = Layout::horizontal([
        Constraint::Length(pipeline_name.len() as u16 + 4),
        Constraint::Min(0),
        Constraint::Max(40),
    ]).split(area);

    // Pipeline Name
    frame.render_widget(
        Paragraph::new(format!(" {} ", pipeline_name.to_uppercase())).bold().black().bg(Color::Cyan),
        chunks[0]
    );

    // Tabs
    let tabs = Tabs::new(AppView::titles())
        .highlight_style(Style::default().bold().underlined().fg(Color::White))
        .select(current_view.to_index())
        .divider("|")
        .padding("  ", "  ");
    frame.render_widget(tabs, chunks[1]);

    // Git info
    if chunks[2].width > 10 {
        let git_p = Paragraph::new(format!(" git:{} ", git_info))
            .dim()
            .alignment(Alignment::Right);
        frame.render_widget(git_p, chunks[2]);
    }
}

fn draw_dashboard(frame: &mut Frame, area: Rect, states: &[JobState], selected_job: usize, log_scroll: u16, search_query: &str) {
    let chunks = Layout::horizontal([
        Constraint::Percentage(25),
        Constraint::Min(0),
    ]).split(area);

    // Job List
    let items: Vec<ListItem> = states
        .iter()
        .enumerate()
        .map(|(i, state)| {
            let (marker, color) = match state.status {
                JobStatus::Pending => (" . ", Color::Gray),
                JobStatus::Running => (" > ", Color::Yellow),
                JobStatus::Success => (" v ", Color::Green),
                JobStatus::Failed => (" x ", Color::Red),
            };

            let mut line = Line::from(vec![
                Span::styled(marker, Style::default().fg(color).bold()),
                Span::from(state.name.clone()),
            ]);

            if i == selected_job {
                line = line.patch_style(Style::default().bg(Color::Rgb(50, 54, 62)).bold());
            }

            ListItem::new(line)
        })
        .collect();

    let job_list = List::new(items)
        .block(Block::default()
            .borders(Borders::RIGHT)
            .border_style(Style::default().dim())
            .title(" JOBS ".bold())
        );
    
    frame.render_widget(job_list, chunks[0]);

    // Logs
    let (logs_text, line_count) = if let Some(state) = states.get(selected_job) {
        if state.logs.is_empty() {
            ("No logs available.".dim().to_string(), 0)
        } else if !search_query.is_empty() {
            let filtered: Vec<String> = state.logs.iter()
                .filter(|l| l.to_lowercase().contains(&search_query.to_lowercase()))
                .cloned()
                .collect();
            let count = filtered.len();
            if filtered.is_empty() {
                (format!("No matches found for '{}'", search_query).italic().yellow().to_string(), 0)
            } else {
                (filtered.join("\n"), count)
            }
        } else {
            (state.logs.join("\n"), state.logs.len())
        }
    } else {
        ("Select a job.".into(), 0)
    };

    let log_title = if let Some(state) = states.get(selected_job) {
        let mut title_spans = vec![
            " LOGS: ".into(),
            state.name.to_uppercase().bold(),
            format!(" [{} lines] ", line_count).dim(),
        ];
        if !search_query.is_empty() {
            title_spans.push(" FILTERED BY: ".yellow());
            title_spans.push(search_query.bold().yellow());
        }
        Line::from(title_spans)
    } else {
        Line::from(" LOGS ".bold())
    };

    let log_view = Paragraph::new(logs_text)
        .block(Block::default().title(log_title))
        .wrap(Wrap { trim: false })
        .scroll((log_scroll, 0));
    
    frame.render_widget(log_view, chunks[1]);
}

fn draw_settings(frame: &mut Frame, area: Rect, pipeline: &Pipeline) {
    let mut rows = Vec::new();

    rows.push(Row::new(vec![
        Cell::from(" [GLOBAL] ").bold().cyan(),
        Cell::from(""),
        Cell::from(""),
    ]).bottom_margin(1));

    if let Some(env) = &pipeline.env {
        for (k, v) in env {
            rows.push(Row::new(vec![
                Cell::from(""),
                Cell::from(k.clone()).yellow(),
                Cell::from(v.clone()).italic().dim(),
            ]));
        }
    }

    rows.push(Row::new(vec![Cell::from(""); 3]));

    for job in &pipeline.jobs {
        rows.push(Row::new(vec![
            Cell::from(format!(" [JOB: {}] ", job.name.to_uppercase())).bold().magenta(),
            Cell::from(""),
            Cell::from(""),
        ]).top_margin(1));
        
        if let Some(env) = &job.env {
            for (k, v) in env {
                rows.push(Row::new(vec![
                    Cell::from(""),
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
    .block(Block::default().padding(Padding::horizontal(1)));

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
    .block(Block::default().padding(Padding::horizontal(1)));

    frame.render_widget(table, area);
}

fn draw_footer(frame: &mut Frame, area: Rect, states: &[JobState], search_query: &str) {
    let success = states.iter().filter(|s| s.status == JobStatus::Success).count();
    let failed = states.iter().filter(|s| s.status == JobStatus::Failed).count();
    let running = states.iter().filter(|s| s.status == JobStatus::Running).count();

    let mut spans = Vec::new();

    // Responsive Help Keys
    if area.width > 90 {
        spans.push(" [q] Quit ".bold().red());
        spans.push(" [1-3] View ".bold().blue());
        spans.push(" [Up/Dn] Job ".bold().yellow());
        spans.push(" [/] Search ".bold().cyan());
        spans.push(" [PgUp/Dn] Scroll ".bold().magenta());
    } else if area.width > 60 {
        spans.push(" [q] Quit ".bold().red());
        spans.push(" [/] Search ".bold().cyan());
        spans.push(" [1-3] View ".bold().blue());
    } else {
        spans.push(" q:Quit ".bold().red());
        spans.push(" /:Find ".bold().cyan());
    }

    spans.push(" | ".dim());

    if !search_query.is_empty() {
        spans.push(" SEARCH: ".bold().yellow());
        spans.push(search_query.bold().white().bg(Color::Rgb(60, 60, 60)));
        spans.push(" [Esc] Clear ".dim());
        spans.push(" | ".dim());
    }

    // Responsive Status Metrics
    if area.width > 70 {
        spans.push(format!(" {} OK ", success).bold().green());
        spans.push(format!(" {} FAIL ", failed).bold().red());
        spans.push(format!(" {} RUN ", running).bold().yellow());
    } else {
        spans.push(format!(" OK:{} ", success).bold().green());
        spans.push(format!(" ERR:{} ", failed).bold().red());
    }
    
    let footer = Paragraph::new(Line::from(spans))
        .bg(Color::Rgb(40, 44, 52))
        .alignment(Alignment::Left);
    frame.render_widget(footer, area);
}
