use crate::pipeline::Pipeline;
use crate::runner::{JobState, JobStatus};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{
        Block, Borders, Cell, Gauge, List, ListItem, Padding, Paragraph, Row, Table, Tabs, Wrap,
    },
};

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
    History,
    Settings,
    EnvVars,
    CredentialsPrompt,
}

impl AppView {
    pub fn to_index(self) -> usize {
        match self {
            AppView::Dashboard => 0,
            AppView::History => 1,
            AppView::Settings => 2,
            AppView::EnvVars => 3,
            AppView::CredentialsPrompt => 4,
        }
    }

    pub fn titles() -> Vec<Line<'static>> {
        vec![
            Line::from(vec![Span::styled(" DASHBOARD ", Style::default().bold())]),
            Line::from(vec![Span::styled(" HISTORY ", Style::default().bold())]),
            Line::from(vec![Span::styled(" PIPELINE ", Style::default().bold())]),
            Line::from(vec![Span::styled(" ENVIRONMENT ", Style::default().bold())]),
        ]
    }
}

#[allow(clippy::too_many_arguments)]
pub fn draw(
    frame: &mut Frame,
    states: &[JobState],
    selected_job: usize,
    git_info: &str,
    pipeline_name: &str,
    current_view: &AppView,
    pipeline: &Pipeline,
    user_env: &std::collections::HashMap<String, String>,
    log_scroll: &mut u16,
    search_query: &str,
    build_id: u32,
    history: &[crate::runner::BuildRecord],
    prompt_secret_name: Option<&str>,
    prompt_buffer: &str,
    history_table_state: &mut ratatui::widgets::TableState,
    is_viewing_history: bool,
    next_run: Option<chrono::DateTime<chrono::Local>>,
) {
    let area = frame.area();

    let constraints = vec![
        Constraint::Length(1), // Top Status Bar
        Constraint::Length(1), // Progress Gauge
        Constraint::Min(0),    // Main Content
        Constraint::Length(1), // Footer
    ];

    let chunks = Layout::vertical(constraints).split(area);

    draw_header(
        frame,
        chunks[0],
        pipeline_name,
        git_info,
        current_view,
        build_id,
        is_viewing_history,
        next_run,
    );
    draw_progress(frame, chunks[1], states);

    let main_area = chunks[2];
    match current_view {
        AppView::Dashboard => draw_dashboard(
            frame,
            main_area,
            states,
            selected_job,
            log_scroll,
            search_query,
        ),
        AppView::History => {
            if is_viewing_history {
                draw_dashboard(
                    frame,
                    main_area,
                    states,
                    selected_job,
                    log_scroll,
                    search_query,
                );
            } else {
                draw_history(frame, main_area, history, history_table_state);
            }
        }
        AppView::Settings => draw_settings(frame, main_area, pipeline),
        AppView::EnvVars => draw_env_vars(frame, main_area, user_env),
        AppView::CredentialsPrompt => draw_credentials_prompt(
            frame,
            main_area,
            prompt_secret_name.unwrap_or(""),
            prompt_buffer,
        ),
    }

    draw_footer(frame, chunks[3], states, search_query, is_viewing_history);

    if let AppView::CredentialsPrompt = current_view {
        // We might want a modal instead of a full screen
    }
}

fn draw_credentials_prompt(frame: &mut Frame, area: Rect, secret_name: &str, buffer: &str) {
    let chunks = Layout::vertical([
        Constraint::Min(0),
        Constraint::Length(10),
        Constraint::Min(0),
    ])
    .split(area);

    let inner_chunks = Layout::horizontal([
        Constraint::Min(0),
        Constraint::Length(60),
        Constraint::Min(0),
    ])
    .split(chunks[1]);

    let block = Block::default()
        .title(Line::from(vec![Span::styled(
            " ⟫ SECURE CREDENTIAL ENTRY ",
            Style::default().bold().fg(CLR_YELLOW),
        )]))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(CLR_YELLOW))
        .padding(Padding::uniform(1));

    let masked_input: String = "*".repeat(buffer.len());

    let content = vec![
        Line::from(vec![Span::styled(
            "Missing required secret: ".to_string(),
            Style::default().fg(CLR_FG),
        )]),
        Line::from(vec![Span::styled(
            secret_name,
            Style::default().bold().fg(CLR_CYAN),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Value: ", Style::default().fg(CLR_GRAY)),
            Span::styled(masked_input, Style::default().fg(CLR_GREEN)),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            " [Enter] Submit  [Esc] Skip ",
            Style::default().dim(),
        )]),
    ];

    let paragraph = Paragraph::new(content)
        .block(block)
        .alignment(Alignment::Center);

    frame.render_widget(paragraph, inner_chunks[1]);
}

#[allow(clippy::too_many_arguments)]
fn draw_header(
    frame: &mut Frame,
    area: Rect,
    pipeline_name: &str,
    git_info: &str,
    current_view: &AppView,
    build_id: u32,
    is_viewing_history: bool,
    next_run: Option<chrono::DateTime<chrono::Local>>,
) {
    let header_constraints = if next_run.is_some() {
        vec![
            Constraint::Length(pipeline_name.len() as u16 + 22),
            Constraint::Length(25),
            Constraint::Min(0),
            Constraint::Max(30),
        ]
    } else {
        vec![
            Constraint::Length(pipeline_name.len() as u16 + 22),
            Constraint::Min(0),
            Constraint::Max(30),
        ]
    };

    let chunks = Layout::horizontal(header_constraints).split(area);

    // App Logo/Name + Build ID
    let title = if is_viewing_history {
        format!(" CONVEYOR ⟫ HISTORICAL #{} ", build_id)
    } else {
        format!(
            " CONVEYOR ⟫ {} #{} ",
            pipeline_name.to_uppercase(),
            build_id
        )
    };

    frame.render_widget(
        Paragraph::new(title)
            .bold()
            .fg(CLR_BG)
            .bg(if is_viewing_history {
                CLR_PURPLE
            } else {
                CLR_CYAN
            }),
        chunks[0],
    );

    let (tabs_idx, git_idx) = if next_run.is_some() {
        if let Some(time) = next_run {
            let next_run_str = format!(" NEXT RUN: {} ", time.format("%H:%M:%S"));
            frame.render_widget(
                Paragraph::new(next_run_str)
                    .fg(CLR_YELLOW)
                    .bold()
                    .alignment(Alignment::Center),
                chunks[1],
            );
        }
        (2, 3)
    } else {
        (1, 2)
    };

    // Modern Navigation Tabs
    let tabs = Tabs::new(AppView::titles())
        .highlight_style(Style::default().fg(CLR_BLUE).bold().underlined())
        .select(current_view.to_index())
        .divider(Span::styled(" │ ", Style::default().fg(CLR_GRAY)))
        .padding(" ", " ");
    frame.render_widget(tabs, chunks[tabs_idx]);

    // Git Status
    if chunks[git_idx].width > 10 {
        let git_p = Paragraph::new(format!(" branch:{} ", git_info))
            .fg(CLR_GRAY)
            .alignment(Alignment::Right);
        frame.render_widget(git_p, chunks[git_idx]);
    }
}

fn draw_progress(frame: &mut Frame, area: Rect, states: &[JobState]) {
    let total = states.len();
    if total == 0 {
        return;
    }

    let finished = states
        .iter()
        .filter(|s| s.status == JobStatus::Success || s.status == JobStatus::Failed)
        .count();
    let ratio = finished as f64 / total as f64;

    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(CLR_GREEN).bg(CLR_GRAY))
        .use_unicode(true)
        .ratio(ratio);

    frame.render_widget(gauge, area);
}

fn draw_dashboard(
    frame: &mut Frame,
    area: Rect,
    states: &[JobState],
    selected_job: usize,
    log_scroll: &mut u16,
    search_query: &str,
) {
    let chunks = Layout::horizontal([Constraint::Percentage(30), Constraint::Min(0)]).split(area);

    // --- SIDEBAR: STAGE & JOB LIST ---
    let mut items: Vec<ListItem> = Vec::new();
    let mut current_stage = String::new();

    for (i, state) in states.iter().enumerate() {
        if state.stage_name != current_stage {
            current_stage = state.stage_name.clone();
            items.push(
                ListItem::new(Line::from(vec![Span::styled(
                    format!(" ⟫ {} ", current_stage.to_uppercase()),
                    Style::default().bold().fg(CLR_BLUE).bg(CLR_SEL_BG),
                )]))
                .style(Style::default().bg(CLR_SEL_BG)),
            );
        }

        let (icon, color) = match state.status {
            JobStatus::Pending => (" • ", CLR_FG),
            JobStatus::Running => (" ⟫ ", CLR_YELLOW),
            JobStatus::Success => (" ✔ ", CLR_GREEN),
            JobStatus::Failed => (" ✘ ", CLR_RED),
        };

        let mut style = Style::default().fg(color);
        let mut name_style = Style::default().fg(CLR_FG);

        if i == selected_job {
            style = style.bg(CLR_SEL_BG).bold();
            name_style = name_style.bg(CLR_SEL_BG).bold().fg(Color::White);
        }

        let line = Line::from(vec![
            Span::styled(format!("  {}", icon), style),
            Span::styled(format!("{:<18}", state.name), name_style),
            Span::styled(
                format!(" {:>6}", state.elapsed()),
                Style::default().fg(CLR_GRAY).bg(if i == selected_job {
                    CLR_SEL_BG
                } else {
                    Color::Reset
                }),
            ),
        ]);

        items.push(ListItem::new(line));
    }

    let job_list = List::new(items).block(
        Block::default()
            .borders(Borders::RIGHT)
            .border_style(Style::default().fg(CLR_GRAY))
            .title(Span::styled(
                " PIPELINE ",
                Style::default().bold().fg(CLR_GRAY),
            ))
            .padding(Padding::uniform(1)),
    );

    frame.render_widget(job_list, chunks[0]);

    // --- MAIN: LOG TERMINAL ---
    let (logs_text, line_count) = if let Some(state) = states.get(selected_job) {
        if state.logs.is_empty() {
            (
                ratatui::text::Text::from("No logs recorded for this task.".italic().fg(CLR_GRAY)),
                0,
            )
        } else if !search_query.is_empty() {
            let mut highlighted_lines = Vec::new();
            let query_lower = search_query.to_lowercase();

            for line in &state.logs {
                if line.to_lowercase().contains(&query_lower) {
                    let mut spans = Vec::new();
                    let mut last_pos = 0;
                    let line_lower = line.to_lowercase();

                    for (index, _) in line_lower.match_indices(&query_lower) {
                        // Text before match
                        if index > last_pos {
                            spans.push(Span::from(&line[last_pos..index]));
                        }
                        // The match itself
                        let end = index + search_query.len();
                        spans.push(Span::styled(
                            &line[index..end],
                            Style::default().fg(Color::Black).bg(CLR_YELLOW).bold(),
                        ));
                        last_pos = end;
                    }

                    // Remaining text
                    if last_pos < line.len() {
                        spans.push(Span::from(&line[last_pos..]));
                    }

                    highlighted_lines.push(Line::from(spans));
                }
            }

            let count = highlighted_lines.len();
            if count == 0 {
                (
                    ratatui::text::Text::from(
                        format!("Pattern '{}' not found in logs.", search_query)
                            .italic()
                            .fg(CLR_YELLOW),
                    ),
                    0,
                )
            } else {
                (ratatui::text::Text::from(highlighted_lines), count)
            }
        } else {
            let lines: Vec<Line> = state.logs.iter().map(|l| Line::from(l.as_str())).collect();
            (ratatui::text::Text::from(lines), state.logs.len())
        }
    } else {
        (
            ratatui::text::Text::from("Select a task from the sidebar."),
            0,
        )
    };

    let log_title = if let Some(state) = states.get(selected_job) {
        let mut title_spans = vec![
            Span::styled(" TERMINAL OUTPUT: ", Style::default().fg(CLR_GRAY)),
            Span::styled(
                state.name.to_uppercase(),
                Style::default().bold().fg(CLR_BLUE),
            ),
            Span::styled(
                format!(" [{} lines]", line_count),
                Style::default().fg(CLR_GRAY),
            ),
        ];
        if !search_query.is_empty() {
            title_spans.push(Span::styled(
                " ⟫ FILTERED BY: ",
                Style::default().fg(CLR_YELLOW),
            ));
            title_spans.push(Span::styled(
                search_query,
                Style::default().bold().fg(CLR_YELLOW).underlined(),
            ));
        }
        Line::from(title_spans)
    } else {
        Line::from(vec![Span::styled(
            " TERMINAL ",
            Style::default().bold().fg(CLR_GRAY),
        )])
    };

    // Calculate visible height for scrolling
    let log_area = chunks[1];
    let visible_height = log_area.height.saturating_sub(3) as usize; // Title + Padding

    if *log_scroll == u16::MAX {
        *log_scroll = line_count.saturating_sub(visible_height) as u16;
    } else {
        // Clamp log_scroll to avoid scrolling past everything
        *log_scroll = (*log_scroll).min(line_count.saturating_sub(1) as u16);
    }

    let log_view = Paragraph::new(logs_text)
        .block(
            Block::default()
                .title(log_title)
                .padding(Padding::new(2, 2, 1, 1)),
        )
        .wrap(Wrap { trim: false })
        .scroll((*log_scroll, 0));

    frame.render_widget(log_view, chunks[1]);
}

fn draw_history(
    frame: &mut Frame,
    area: Rect,
    history: &[crate::runner::BuildRecord],
    state: &mut ratatui::widgets::TableState,
) {
    if history.is_empty() {
        frame.render_widget(
            Paragraph::new("No build history found. Run a pipeline to see results here.")
                .alignment(Alignment::Center)
                .fg(CLR_GRAY)
                .block(Block::default().padding(Padding::uniform(5))),
            area,
        );
        return;
    }

    let mut rows = Vec::new();
    for build in history {
        let status_style = match build.status {
            JobStatus::Success => Style::default().fg(CLR_GREEN).bold(),
            JobStatus::Failed => Style::default().fg(CLR_RED).bold(),
            _ => Style::default().fg(CLR_YELLOW),
        };

        rows.push(Row::new(vec![
            Cell::from(format!("#{}", build.id)).fg(CLR_CYAN),
            Cell::from(build.timestamp.format("%Y-%m-%d %H:%M:%S").to_string()).fg(CLR_FG),
            Cell::from(format!("{:?}", build.status)).style(status_style),
            Cell::from(format!("{} jobs", build.jobs.len())).fg(CLR_GRAY),
        ]));
    }

    let table = Table::new(
        rows,
        [
            Constraint::Length(8),
            Constraint::Length(20),
            Constraint::Length(12),
            Constraint::Min(10),
        ],
    )
    .header(
        Row::new(vec!["ID", "TIMESTAMP", "STATUS", "DETAILS"])
            .style(Style::default().bold().fg(CLR_GRAY))
            .bottom_margin(1),
    )
    .block(Block::default().padding(Padding::uniform(2)))
    .row_highlight_style(Style::default().bg(CLR_SEL_BG).bold())
    .highlight_symbol(" ⟫ ");

    frame.render_stateful_widget(table, area, state);
}

fn draw_settings(frame: &mut Frame, area: Rect, pipeline: &Pipeline) {
    let mut rows = Vec::new();

    rows.push(
        Row::new(vec![
            Cell::from(" ⟫ GLOBAL CONFIG ").bold().fg(CLR_CYAN),
            Cell::from(""),
            Cell::from(""),
        ])
        .bottom_margin(1),
    );

    if let Some(env) = &pipeline.env {
        for (k, v) in env {
            rows.push(Row::new(vec![
                Cell::from("   env").fg(CLR_GRAY),
                Cell::from(k.clone()).fg(CLR_YELLOW),
                Cell::from(v.clone()).italic().fg(CLR_FG),
            ]));
        }
    }

    let all_jobs = if let Some(stages) = &pipeline.stages {
        stages
            .iter()
            .flat_map(|s| s.jobs.iter())
            .collect::<Vec<_>>()
    } else if let Some(jobs) = &pipeline.jobs {
        jobs.iter().collect()
    } else {
        Vec::new()
    };

    for job in all_jobs {
        rows.push(
            Row::new(vec![
                Cell::from(format!(" ⟫ JOB: {}", job.name.to_uppercase()))
                    .bold()
                    .fg(CLR_PURPLE),
                Cell::from(""),
                Cell::from(""),
            ])
            .top_margin(1),
        );

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
        [Constraint::Percentage(40), Constraint::Percentage(60)],
    )
    .header(
        Row::new(vec!["LOCAL VARIABLE", "CURRENT VALUE"])
            .style(Style::default().bold().fg(CLR_GRAY))
            .bottom_margin(1),
    )
    .block(Block::default().padding(Padding::uniform(2)));

    frame.render_widget(table, area);
}

fn draw_footer(
    frame: &mut Frame,
    area: Rect,
    states: &[JobState],
    search_query: &str,
    is_viewing_history: bool,
) {
    let success = states
        .iter()
        .filter(|s| s.status == JobStatus::Success)
        .count();
    let failed = states
        .iter()
        .filter(|s| s.status == JobStatus::Failed)
        .count();
    let running = states
        .iter()
        .filter(|s| s.status == JobStatus::Running)
        .count();

    let mut spans = Vec::new();

    // Contextual Help
    if is_viewing_history {
        spans.push(Span::styled(
            " [Esc] Back to List ",
            Style::default().fg(CLR_BLUE),
        ));
        spans.push(Span::styled(" [↑↓] Job ", Style::default().fg(CLR_YELLOW)));
    } else {
        spans.push(Span::styled(" [1-4] View ", Style::default().fg(CLR_BLUE)));
        spans.push(Span::styled(" [↑↓] Task ", Style::default().fg(CLR_YELLOW)));
        spans.push(Span::styled(" [/] Find ", Style::default().fg(CLR_CYAN)));
        spans.push(Span::styled(" [R] Retry ", Style::default().fg(CLR_GREEN)));
    }

    if area.width > 100 {
        spans.push(Span::styled(
            " [PgUp/Dn] Log ",
            Style::default().fg(CLR_PURPLE),
        ));
        spans.push(Span::styled(" [Q] Quit ", Style::default().fg(CLR_RED)));
    }

    spans.push(Span::styled(" │ ", Style::default().fg(CLR_GRAY)));

    if !search_query.is_empty() {
        spans.push(Span::styled(
            " FILTER: ",
            Style::default().bold().fg(CLR_YELLOW),
        ));
        spans.push(Span::styled(
            search_query,
            Style::default().bg(CLR_SEL_BG).fg(Color::White),
        ));
        spans.push(Span::styled(" [Esc] Clear ", Style::default().fg(CLR_GRAY)));
        spans.push(Span::styled(" │ ", Style::default().fg(CLR_GRAY)));
    }

    // Stats
    spans.push(Span::styled(
        format!(" {} PASSED ", success),
        Style::default().fg(CLR_GREEN).bold(),
    ));
    spans.push(Span::styled(
        format!(" {} FAILED ", failed),
        Style::default().fg(CLR_RED).bold(),
    ));
    spans.push(Span::styled(
        format!(" {} ACTIVE ", running),
        Style::default().fg(CLR_YELLOW).bold(),
    ));

    let footer = Paragraph::new(Line::from(spans))
        .bg(CLR_SEL_BG)
        .alignment(Alignment::Left);
    frame.render_widget(footer, area);
}
