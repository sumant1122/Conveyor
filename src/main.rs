mod pipeline;
mod runner;
mod ui;

use crate::pipeline::Pipeline;
use crate::runner::Runner;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::env;
use std::io;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn get_git_info() -> String {
    let branch = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown branch".to_string());

    let commit = Command::new("git")
        .args(["log", "-1", "--format=%h - %s"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "no commits".to_string());

    format!("{} ({})", branch, commit)
}

use crate::ui::AppView;

fn setup_terminal() -> anyhow::Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    Ok(Terminal::new(backend)?)
}

async fn run_headless(
    pipeline: Pipeline,
    user_env: std::collections::HashMap<String, String>,
    secrets: std::collections::HashMap<String, String>,
) -> anyhow::Result<()> {
    use crate::runner::JobStatus;

    let runner = Arc::new(Runner::new(
        pipeline.clone(),
        user_env,
        secrets,
        pipeline.repository.is_some(),
    ));
    println!("🚀 Starting Conveyor in headless mode...");
    println!("📋 Pipeline: {}", runner.pipeline.lock().await.name);
    println!("-------------------------------------------");

    let r_clone = runner.clone();
    tokio::spawn(async move {
        r_clone.run().await;
    });

    let mut last_log_indices: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut finished_jobs: std::collections::HashSet<String> = std::collections::HashSet::new();

    loop {
        let states = runner.states.lock().await.clone();
        let mut all_finished = true;
        let mut any_failed = false;

        for state in &states {
            let logs = &state.logs;
            let last_idx = *last_log_indices.get(&state.name).unwrap_or(&0);

            if last_idx < logs.len() {
                for log in logs.iter().skip(last_idx) {
                    println!("[{}] {}", state.name, log);
                }
                last_log_indices.insert(state.name.clone(), logs.len());
            }

            if state.status == JobStatus::Success || state.status == JobStatus::Failed {
                if !finished_jobs.contains(&state.name) {
                    println!(
                        "✅ Job '{}' finished with status: {:?}",
                        state.name, state.status
                    );
                    finished_jobs.insert(state.name.clone());
                }
                if state.status == JobStatus::Failed {
                    any_failed = true;
                }
            } else {
                all_finished = false;
            }
        }

        if all_finished && !states.is_empty() {
            println!("-------------------------------------------");
            if any_failed {
                println!("❌ Pipeline FAILED");
                std::process::exit(1);
            } else {
                println!("✨ Pipeline SUCCESS");
                return Ok(());
            }
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

fn restore_terminal(mut terminal: Terminal<CrosstermBackend<io::Stdout>>) -> anyhow::Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args: Vec<String> = env::args().collect();
    let is_headless = args.iter().any(|a| a == "--headless");
    args.retain(|a| a != "--headless");

    let mut custom_file = None;
    if let Some(pos) = args
        .iter()
        .position(|a| a == "-f" || a == "--file")
        .filter(|&pos| pos + 1 < args.len())
    {
        custom_file = Some(args[pos + 1].clone());
        args.remove(pos + 1);
        args.remove(pos);
    }

    // Load user environment variables
    let env_content = tokio::fs::read_to_string("env.yaml")
        .await
        .unwrap_or_else(|_| {
            let default = "API_KEY_PUBLIC: \"your-public-key-here\"\nDEBUG: \"true\"\n";
            std::fs::write("env.yaml", default).unwrap();
            default.to_string()
        });
    let user_env: std::collections::HashMap<String, String> =
        serde_yaml::from_str(&env_content).unwrap_or_default();

    // Load secrets
    let secrets_content = tokio::fs::read_to_string("secrets.yaml")
        .await
        .unwrap_or_else(|_| {
            let default = "SSH_KEY: \"\"\nAPI_TOKEN: \"\"\n";
            std::fs::write("secrets.yaml", default).unwrap();
            default.to_string()
        });
    let mut secrets: std::collections::HashMap<String, String> =
        serde_yaml::from_str(&secrets_content).unwrap_or_default();

    let pipeline = if args.len() > 1 {
        Pipeline {
            name: "Remote Pipeline".to_string(),
            repository: Some(args[1].clone()),
            branch: args.get(2).cloned(),
            schedule: None,
            env: None,
            secrets: None,
            on_success: None,
            on_failure: None,
            concurrency: None,
            jobs: Some(Vec::new()),
            stages: None,
        }
    } else {
        let path = custom_file.unwrap_or_else(|| "pipeline.yaml".to_string());
        let content = if std::path::Path::new(&path).exists() {
            tokio::fs::read_to_string(&path).await?
        } else if path == "pipeline.yaml" {
            let default = r#"name: Conveyor Build
secrets:
  - SSH_KEY
jobs:
  - name: Build
    command: cargo build
"#;
            std::fs::write("pipeline.yaml", default).unwrap();
            default.to_string()
        } else {
            anyhow::bail!("Specified pipeline file '{}' not found", path);
        };
        Pipeline::from_yaml(&content)?
    };

    // Check for missing secrets
    let mut missing_secrets = Vec::new();
    if let Some(required) = &pipeline.secrets {
        for s in required {
            if secrets.get(s).map(|v| v.is_empty()).unwrap_or(true) {
                missing_secrets.push(s.clone());
            }
        }
    }

    // In headless mode, we can't prompt interactively via TUI
    if is_headless && !missing_secrets.is_empty() {
        eprintln!(
            "Error: Missing required secrets in headless mode: {:?}",
            missing_secrets
        );
        eprintln!("Please provide them in secrets.yaml or run without --headless once.");
        std::process::exit(1);
    }

    if is_headless {
        return run_headless(pipeline, user_env, secrets).await;
    }

    let git_info = if let Some(repo) = &pipeline.repository {
        format!("Remote: {}", repo)
    } else {
        get_git_info()
    };

    let mut current_view = if !missing_secrets.is_empty() {
        AppView::CredentialsPrompt
    } else {
        AppView::Dashboard
    };

    let mut prompt_buffer = String::new();
    let mut current_missing_index = 0;

    let user_env_ui = user_env.clone();
    let mut runner = None;
    let mut runner_started = false;

    if missing_secrets.is_empty() {
        let r = Arc::new(Runner::new(
            pipeline.clone(),
            user_env.clone(),
            secrets.clone(),
            pipeline.repository.is_some(),
        ));
        let r_spawn = r.clone();
        tokio::spawn(async move {
            r_spawn.run().await;
        });
        runner = Some(r);
        runner_started = true;
    }

    let mut terminal = setup_terminal()?;

    let mut selected_job = 0;
    let mut log_scroll: u16 = 0;
    let mut search_query = String::new();
    let mut is_searching = false;

    // History selection state
    let mut history_table_state = ratatui::widgets::TableState::default();
    history_table_state.select(Some(0));
    let mut viewing_history: Option<crate::runner::BuildRecord> = None;

    // Cron state
    let mut next_run: Option<chrono::DateTime<chrono::Local>> = None;
    if let Some(schedule) = pipeline
        .schedule
        .as_ref()
        .and_then(|s| s.parse::<cron::Schedule>().ok())
    {
        next_run = schedule.upcoming(chrono::Local).next();
    }

    let tick_rate = Duration::from_millis(100);
    let mut last_tick = Instant::now();

    let mut current_git_info = git_info;
    let mut git_update_tick = Instant::now();

    let mut last_log_counts: std::collections::HashMap<usize, usize> =
        std::collections::HashMap::new();

    loop {
        // Check cron trigger
        if let Some(r) = next_run
            .filter(|&time| chrono::Local::now() >= time)
            .and(runner.as_ref())
        {
            r.cancel_token.cancel();
            let r_clone = r.clone();
            let pipeline_clone = pipeline.clone();
            tokio::spawn(async move {
                r_clone.reset().await;
                r_clone.run().await;
            });

            if let Some(schedule) = pipeline_clone
                .schedule
                .as_ref()
                .and_then(|s| s.parse::<cron::Schedule>().ok())
            {
                next_run = schedule.upcoming(chrono::Local).next();
            }
        }

        let mut current_build_id = 0;

        let history_records = if current_view == AppView::History {
            if let Some(r) = &runner {
                r.history.load_history()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        let states_guard;
        let pipeline_guard;
        let states_ref: &[crate::runner::JobState];
        let p_name: String;
        let p_config: Pipeline;

        if let Some(r) = &runner {
            let states = r.states.lock().await;
            let p = r.pipeline.lock().await;

            if let Some(state) = states.get(selected_job) {
                let current_count = state.logs.len();
                let last_count = *last_log_counts.get(&selected_job).unwrap_or(&0);

                if current_count > last_count {
                    if log_scroll + 20 >= last_count as u16 {
                        log_scroll = u16::MAX;
                    }
                    last_log_counts.insert(selected_job, current_count);
                }
            }

            p_name = p.name.clone();
            p_config = p.clone();
            current_build_id = r.build_id;

            states_guard = Some(states);
            pipeline_guard = Some(p);
            states_ref = states_guard.as_ref().unwrap().as_slice();
        } else {
            states_guard = None;
            pipeline_guard = None;
            states_ref = &[];
            p_name = pipeline.name.clone();
            p_config = pipeline.clone();
        }

        if p_config.repository.is_none() && git_update_tick.elapsed() >= Duration::from_secs(5) {
            current_git_info = get_git_info();
            git_update_tick = Instant::now();
        }

        let prompt_name = missing_secrets
            .get(current_missing_index)
            .map(|s| s.as_str());

        let states_to_draw = if let Some(record) = &viewing_history {
            &record.jobs
        } else {
            states_ref
        };

        let active_build_id = if let Some(record) = &viewing_history {
            record.id
        } else {
            current_build_id
        };

        terminal.draw(|f| {
            ui::draw(
                f,
                states_to_draw,
                selected_job,
                &current_git_info,
                &p_name,
                &current_view,
                &p_config,
                &user_env_ui,
                &mut log_scroll,
                &search_query,
                active_build_id,
                &history_records,
                prompt_name,
                &prompt_buffer,
                &mut history_table_state,
                viewing_history.is_some(),
                next_run,
            )
        })?;

        let states_len = states_ref.len();
        drop(states_guard);
        drop(pipeline_guard);

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if event::poll(timeout)? {
            let ev = event::read()?;

            if let Event::Mouse(mouse) = ev {
                match mouse.kind {
                    event::MouseEventKind::ScrollUp => log_scroll = log_scroll.saturating_sub(3),
                    event::MouseEventKind::ScrollDown => log_scroll = log_scroll.saturating_add(3),
                    _ => {}
                }
            }

            if let Event::Key(key) = ev {
                if current_view == AppView::CredentialsPrompt {
                    match key.code {
                        KeyCode::Enter => {
                            if let Some(name) = missing_secrets.get(current_missing_index) {
                                secrets.insert(name.clone(), prompt_buffer.clone());
                                prompt_buffer.clear();
                                current_missing_index += 1;
                                if current_missing_index >= missing_secrets.len() {
                                    current_view = AppView::Dashboard;
                                    // Start runner
                                    let r = Arc::new(Runner::new(
                                        pipeline.clone(),
                                        user_env.clone(),
                                        secrets.clone(),
                                        pipeline.repository.is_some(),
                                    ));
                                    let r_spawn = r.clone();
                                    tokio::spawn(async move {
                                        r_spawn.run().await;
                                    });
                                    runner = Some(r);
                                    runner_started = true;
                                }
                            }
                        }
                        KeyCode::Esc => {
                            current_missing_index += 1;
                            prompt_buffer.clear();
                            if current_missing_index >= missing_secrets.len() {
                                current_view = AppView::Dashboard;
                                if !runner_started {
                                    let r = Arc::new(Runner::new(
                                        pipeline.clone(),
                                        user_env.clone(),
                                        secrets.clone(),
                                        pipeline.repository.is_some(),
                                    ));
                                    let r_spawn = r.clone();
                                    tokio::spawn(async move {
                                        r_spawn.run().await;
                                    });
                                    runner = Some(r);
                                    runner_started = true;
                                }
                            }
                        }
                        KeyCode::Backspace => {
                            prompt_buffer.pop();
                        }
                        KeyCode::Char(c) => {
                            prompt_buffer.push(c);
                        }
                        _ => {}
                    }
                } else if is_searching {
                    match key.code {
                        KeyCode::Esc | KeyCode::Enter => is_searching = false,
                        KeyCode::Backspace => {
                            search_query.pop();
                        }
                        KeyCode::Char(c) => {
                            search_query.push(c);
                        }
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Char('q') => {
                            if let Some(r) = &runner {
                                r.cancel_token.cancel();
                            }
                            break;
                        }
                        KeyCode::Char('/') => {
                            is_searching = true;
                            search_query.clear();
                        }
                        KeyCode::Char('1') => {
                            current_view = AppView::Dashboard;
                            viewing_history = None;
                        }
                        KeyCode::Char('2') => {
                            current_view = AppView::History;
                            viewing_history = None;
                        }
                        KeyCode::Char('3') => current_view = AppView::Settings,
                        KeyCode::Char('4') => current_view = AppView::EnvVars,
                        KeyCode::Char('r') => {
                            if let Some(r) = &runner {
                                r.cancel_token.cancel();
                                let r_clone = r.clone();
                                tokio::spawn(async move {
                                    r_clone.reset().await;
                                    r_clone.run().await;
                                });
                            }
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            if current_view == AppView::Dashboard || viewing_history.is_some() {
                                if selected_job > 0 {
                                    selected_job -= 1;
                                    log_scroll = 0;
                                }
                            } else if current_view == AppView::History {
                                let i = match history_table_state.selected() {
                                    Some(i) => {
                                        if i > 0 {
                                            i - 1
                                        } else {
                                            i
                                        }
                                    }
                                    None => 0,
                                };
                                history_table_state.select(Some(i));
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            if current_view == AppView::Dashboard || viewing_history.is_some() {
                                let max = if let Some(h) = &viewing_history {
                                    h.jobs.len()
                                } else {
                                    states_len
                                };
                                if selected_job < max.saturating_sub(1) {
                                    selected_job += 1;
                                    log_scroll = 0;
                                }
                            } else if current_view == AppView::History {
                                let i = match history_table_state.selected() {
                                    Some(i) => {
                                        if i < history_records.len().saturating_sub(1) {
                                            i + 1
                                        } else {
                                            i
                                        }
                                    }
                                    None => 0,
                                };
                                history_table_state.select(Some(i));
                            }
                        }
                        KeyCode::Enter if current_view == AppView::History => {
                            if let Some(record) = history_table_state
                                .selected()
                                .and_then(|i| history_records.get(i))
                            {
                                viewing_history = Some(record.clone());
                                selected_job = 0;
                                log_scroll = 0;
                            }
                        }
                        KeyCode::Esc => {
                            if viewing_history.is_some() {
                                viewing_history = None;
                            } else {
                                search_query.clear();
                            }
                        }
                        KeyCode::PageUp => log_scroll = log_scroll.saturating_sub(15),
                        KeyCode::PageDown => log_scroll = log_scroll.saturating_add(15),
                        KeyCode::Home => log_scroll = 0,
                        KeyCode::End => log_scroll = u16::MAX,
                        _ => {}
                    }
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }

    restore_terminal(terminal)?;
    Ok(())
}
