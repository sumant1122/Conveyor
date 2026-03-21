use ratatui::{
    layout::{Constraint, Layout, Rect, Alignment},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, List, ListItem, Paragraph, Wrap, Table, Row, Tabs, Cell, Padding, Borders, Gauge},
    Frame,
};
use crate::runner::{JobState, JobStatus};
use crate::pipeline::Pipeline;

// --- MODERN COLOR PALETTE (OneDark Inspired) ---
const CLR_BG: Color = Color::Rgb(40, 44, 52);
const CLR_FG: Color = Color::Rgb(171, 178, 191);
const CLR_CYAN: Color = Color::Rgb(86, 182, 194);
const CLR_BLUE: Color = Color::Rgb(97, 175, 239);
const CLR_PURPLE: Color = Color::Rgb(198, 120, 221);
const CLR_GREEN: Color = Color::Rgb(152, 195, 121);
const CLR_RED: Color = Color::Rgb(224, 108, 117);
const CLR_YELLOW: Color = Color::Rgb(229, 192, 123);
const CLR_GRAY: Color = Color::Rgb(75, 82, 99);
const CLR_SEL_BG: Color = Color::Rgb(62, 68, 81);

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
            Line::from(vec![Span::styled(" DASHBOARD ", Style::default().bold())]),
            Line::from(vec![Span::styled(" PIPELINE ", Style::default().bold())]),
            Line::from(vec![Span::styled(" ENVIRONMENT ", Style::default().bold())]),
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
        Constraint::Length(1), // Top Status Bar
        Constraint::Length(1), // Progress Gauge
        Constraint::Min(0),    // Main Content
        Constraint::Length(1), // Footer
    ];

    let chunks = Layout::vertical(constraints).split(area);

    draw_header(frame, chunks[0], pipeline_name, git_info, current_view);
    draw_progress(frame, chunks[1], states);
    
    let main_area = chunks[2];
    match current_view {
        AppView::Dashboard => draw_dashboard(frame, main_area, states, selected_job, log_scroll, search_query),
        AppView::Settings => draw_settings(frame, main_area, pipeline),
        AppView::EnvVars => draw_env_vars(frame, main_area, user_env),
    }

    draw_footer(frame, chunks[3], states, search_query);
}

fn draw_header(frame: &mut Frame, area: Rect, pipeline_name: &str, git_info: &str, current_view: &AppView) {
    let chunks = Layout::horizontal([
        Constraint::Length(pipeline_name.len() as u16 + 4),
        Constraint::Min(0),
        Constraint::Max(40),
    ]).split(area);

    // App Logo/Name
    frame.render_widget(
        Paragraph::new(format!(" CONVEYOR ⟫ {} ", pipeline_name.to_uppercase()))
            .bold()
            .fg(CLR_BG)
            .bg(CLR_CYAN),
        chunks[0]
    );

    // Modern Navigation Tabs
    let tabs = Tabs::new(AppView::titles())
        .highlight_style(Style::default().fg(CLR_BLUE).bold().underlined())
        .select(current_view.to_index())
        .divider(Span::styled(" │ ", Style::default().fg(CLR_GRAY)))
        .padding(" ", " ");
    frame.render_widget(tabs, chunks[1]);

    // Git Status
    if chunks[2].width > 10 {
        let git_p = Paragraph::new(format!(" branch:{} ", git_info))
            .fg(CLR_GRAY)
            .alignment(Alignment::Right);
        frame.render_widget(git_p, chunks[2]);
    }
}

fn draw_progress(frame: &mut Frame, area: Rect, states: &[JobState]) {
    let total = states.len();
    if total == 0 { return; }
    
    let finished = states.iter().filter(|s| s.status == JobStatus::Success || s.status == JobStatus::Failed).count();
    let ratio = finished as f64 / total as f64;
    
    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(CLR_GREEN).bg(CLR_GRAY))
        .use_unicode(true)
        .ratio(ratio);
        
    frame.render_widget(gauge, area);
}

fn draw_dashboard(frame: &mut Frame, area: Rect, states: &[JobState], selected_job: usize, log_scroll: u16, search_query: &str) {
    let chunks = Layout::horizontal([
        Constraint::Percentage(30),
        Constraint::Min(0),
    ]).split(area);

    // --- SIDEBAR: JOB LIST ---
    let items: Vec<ListItem> = states
        .iter()
        .enumerate()
        .map(|(i, state)| {
            let (icon, color, bg) = match state.status {
                JobStatus::Pending => (" • ", CLR_FG, Color::Reset),
                JobStatus::Running => (" ⟫ ", CLR_YELLOW, Color::Reset),
                JobStatus::Success => (" ✔ ", CLR_GREEN, Color::Reset),
                JobStatus::Failed => (" ✘ ", CLR_RED, Color::Reset),
            };

            let mut style = Style::default().fg(color);
            let mut name_style = Style::default().fg(CLR_FG);
            
            if i == selected_job {
                style = style.bg(CLR_SEL_BG).bold();
                name_style = name_style.bg(CLR_SEL_BG).bold().fg(Color::White);
            }

            let line = Line::from(vec![
                Span::styled(icon, style),
                Span::styled(format!("{:<20}", state.name), name_style),
                Span::styled(format!(" {:>6}", state.elapsed()), Style::default().fg(CLR_GRAY).bg(if i == selected_job { CLR_SEL_BG } else { Color::Reset })),
            ]);

            ListItem::new(line).style(Style::default().bg(bg))
        })
        .collect();

    let job_list = List::new(items)
        .block(Block::default()
            .borders(Borders::RIGHT)
            .border_style(Style::default().fg(CLR_GRAY))
            .title(Span::styled(" PIPELINE JOBS ", Style::default().bold().fg(CLR_GRAY)))
            .padding(Padding::uniform(1))
        );
    
    frame.render_widget(job_list, chunks[0]);

    // --- MAIN: LOG TERMINAL ---
    let (logs_text, line_count) = if let Some(state) = states.get(selected_job) {
        if state.logs.is_empty() {
            ("No logs recorded for this task.".italic().fg(CLR_GRAY).to_string(), 0)
        } else if !search_query.is_empty() {
            let filtered: Vec<String> = state.logs.iter()
                .filter(|l| l.to_lowercase().contains(&search_query.to_lowercase()))
                .cloned()
                .collect();
            let count = filtered.len();
            if filtered.is_empty() {
                (format!("Pattern '{}' not found in logs.", search_query).italic().fg(CLR_YELLOW).to_string(), 0)
            } else {
                (filtered.join("\n"), count)
            }
        } else {
            (state.logs.join("\n"), state.logs.len())
        }
    } else {
        ("Select a task from the sidebar.".into(), 0)
    };

    let log_title = if let Some(state) = states.get(selected_job) {
        let mut title_spans = vec![
            Span::styled(" TERMINAL OUTPUT: ", Style::default().fg(CLR_GRAY)),
            Span::styled(state.name.to_uppercase(), Style::default().bold().fg(CLR_BLUE)),
            Span::styled(format!(" [{} lines]", line_count), Style::default().fg(CLR_GRAY)),
        ];
        if !search_query.is_empty() {
            title_spans.push(Span::styled(" ⟫ FILTERED BY: ", Style::default().fg(CLR_YELLOW)));
            title_spans.push(Span::styled(search_query, Style::default().bold().fg(CLR_YELLOW).underlined()));
        }
        Line::from(title_spans)
    } else {
        Line::from(vec![Span::styled(" TERMINAL ", Style::default().bold().fg(CLR_GRAY))])
    };

    let log_view = Paragraph::new(logs_text)
        .block(Block::default()
            .title(log_title)
            .padding(Padding::new(2, 2, 1, 1))
        )
        .wrap(Wrap { trim: false })
        .scroll((log_scroll, 0));
    
    frame.render_widget(log_view, chunks[1]);
}

fn draw_settings(frame: &mut Frame, area: Rect, pipeline: &Pipeline) {
    let mut rows = Vec::new();

    rows.push(Row::new(vec![
        Cell::from(" ⟫ GLOBAL CONFIG ").bold().fg(CLR_CYAN),
        Cell::from(""),
        Cell::from(""),
    ]).bottom_margin(1));

    if let Some(env) = &pipeline.env {
        for (k, v) in env {
            rows.push(Row::new(vec![
                Cell::from("   env").fg(CLR_GRAY),
                Cell::from(k.clone()).fg(CLR_YELLOW),
                Cell::from(v.clone()).italic().fg(CLR_FG),
            ]));
        }
    }

    for job in &pipeline.jobs {
        rows.push(Row::new(vec![
            Cell::from(format!(" ⟫ JOB: {}", job.name.to_uppercase())).bold().fg(CLR_PURPLE),
            Cell::from(""),
            Cell::from(""),
        ]).top_margin(1));
        
        if let Some(env) = &job.env {
            for (k, v) in env {
                rows.push(Row::new(vec![
                    Cell::from("   env").fg(CLR_GRAY),
                    Cell::from(k.clone()).fg(CLR_YELLOW),
                    Cell::from(v.clone()).italic().fg(CLR_FG),
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
        Row::new(vec!["SCOPE", "KEY", "VALUE"])
            .style(Style::default().bold().fg(CLR_GRAY))
            .bottom_margin(1),
    )
    .block(Block::default().padding(Padding::uniform(2)));

    frame.render_widget(table, area);
}

fn draw_env_vars(frame: &mut Frame, area: Rect, env: &std::collections::HashMap<String, String>) {
    let mut rows = Vec::new();
    let mut keys: Vec<_> = env.keys().collect();
    keys.sort();

    for k in keys {
        rows.push(Row::new(vec![
            Cell::from(k.clone()).bold().fg(CLR_YELLOW),
            Cell::from(env.get(k).unwrap().clone()).italic().fg(CLR_FG),
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
        Row::new(vec!["LOCAL VARIABLE", "CURRENT VALUE"])
            .style(Style::default().bold().fg(CLR_GRAY))
            .bottom_margin(1),
    )
    .block(Block::default().padding(Padding::uniform(2)));

    frame.render_widget(table, area);
}

fn draw_footer(frame: &mut Frame, area: Rect, states: &[JobState], search_query: &str) {
    let success = states.iter().filter(|s| s.status == JobStatus::Success).count();
    let failed = states.iter().filter(|s| s.status == JobStatus::Failed).count();
    let running = states.iter().filter(|s| s.status == JobStatus::Running).count();

    let mut spans = Vec::new();

    // Contextual Help
    if area.width > 100 {
        spans.push(Span::styled(" [1-3] View ", Style::default().fg(CLR_BLUE)));
        spans.push(Span::styled(" [↑↓] Task ", Style::default().fg(CLR_YELLOW)));
        spans.push(Span::styled(" [/] Find ", Style::default().fg(CLR_CYAN)));
        spans.push(Span::styled(" [PgUp/Dn] Log ", Style::default().fg(CLR_PURPLE)));
        spans.push(Span::styled(" [Q] Quit ", Style::default().fg(CLR_RED)));
    } else {
        spans.push(Span::styled(" [1-3] View ", Style::default().fg(CLR_BLUE)));
        spans.push(Span::styled(" [/] Find ", Style::default().fg(CLR_CYAN)));
        spans.push(Span::styled(" [Q] Quit ", Style::default().fg(CLR_RED)));
    }

    spans.push(Span::styled(" │ ", Style::default().fg(CLR_GRAY)));

    if !search_query.is_empty() {
        spans.push(Span::styled(" FILTER: ", Style::default().bold().fg(CLR_YELLOW)));
        spans.push(Span::styled(search_query, Style::default().bg(CLR_SEL_BG).fg(Color::White)));
        spans.push(Span::styled(" [Esc] Clear ", Style::default().fg(CLR_GRAY)));
        spans.push(Span::styled(" │ ", Style::default().fg(CLR_GRAY)));
    }

    // Stats
    spans.push(Span::styled(format!(" {} PASSED ", success), Style::default().fg(CLR_GREEN).bold()));
    spans.push(Span::styled(format!(" {} FAILED ", failed), Style::default().fg(CLR_RED).bold()));
    spans.push(Span::styled(format!(" {} ACTIVE ", running), Style::default().fg(CLR_YELLOW).bold()));
    
    let footer = Paragraph::new(Line::from(spans))
        .bg(CLR_SEL_BG)
        .alignment(Alignment::Left);
    frame.render_widget(footer, area);
}
