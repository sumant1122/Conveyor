mod pipeline;
mod runner;
mod ui;

use crate::pipeline::Pipeline;
use crate::runner::Runner;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::{Duration, Instant};
use std::process::Command;
use std::env;
use std::sync::Arc;

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
    let args: Vec<String> = env::args().collect();
    
    // Load user environment variables
    let env_content = tokio::fs::read_to_string("env.yaml")
        .await
        .unwrap_or_else(|_| {
            let default = "API_KEY_PUBLIC: \"your-public-key-here\"\nDEBUG: \"true\"\n";
            std::fs::write("env.yaml", default).unwrap();
            default.to_string()
        });
    let user_env: std::collections::HashMap<String, String> = serde_yaml::from_str(&env_content).unwrap_or_default();

    // Load secrets
    let secrets_content = tokio::fs::read_to_string("secrets.yaml")
        .await
        .unwrap_or_else(|_| {
            let default = "SSH_KEY: \"\"\nAPI_TOKEN: \"\"\n";
            std::fs::write("secrets.yaml", default).unwrap();
            default.to_string()
        });
    let mut secrets: std::collections::HashMap<String, String> = serde_yaml::from_str(&secrets_content).unwrap_or_default();

    let pipeline = if args.len() > 1 {
        Pipeline {
            name: "Remote Pipeline".to_string(),
            repository: Some(args[1].clone()),
            branch: args.get(2).cloned(),
            env: None,
            secrets: None,
            on_success: None,
            on_failure: None,
            concurrency: None,
            jobs: Some(Vec::new()),
            stages: None,
        }
    } else {
        let content = tokio::fs::read_to_string("pipeline.yaml")
            .await
            .unwrap_or_else(|_| {
                let default = r#"name: Conveyor Build
secrets:
  - SSH_KEY
jobs:
  - name: Build
    steps:
      - name: Compile
        command: cargo build
"#;
                std::fs::write("pipeline.yaml", default).unwrap();
                default.to_string()
            });
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

    let git_info = if pipeline.repository.is_some() {
        format!("Remote: {}", pipeline.repository.as_ref().unwrap())
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
        let r = Arc::new(Runner::new(pipeline.clone(), user_env.clone(), secrets.clone()));
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

    let tick_rate = Duration::from_millis(100);
    let mut last_tick = Instant::now();
    
    let mut current_git_info = git_info;
    let mut git_update_tick = Instant::now();

    let mut last_log_counts: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();

    loop {
        let mut states = Vec::new();
        let mut pipeline_config = pipeline.clone();
        let mut current_build_id = 0;
        let mut history_records = Vec::new();

        if let Some(r) = &runner {
            let s = r.states.lock().await;
            let p = r.pipeline.lock().await;
            states = s.clone();
            pipeline_config = p.clone();
            current_build_id = r.build_id;
            history_records = r.history.load_history();
        }

        // Auto-scroll logic
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

        if pipeline_config.repository.is_none() && git_update_tick.elapsed() >= Duration::from_secs(5) {
             current_git_info = get_git_info();
             git_update_tick = Instant::now();
        }

        let prompt_name = missing_secrets.get(current_missing_index).map(|s| s.as_str());

        terminal.draw(|f| ui::draw(
            f, 
            &states, 
            selected_job, 
            &current_git_info, 
            &pipeline_config.name, 
            &current_view, 
            &pipeline_config, 
            &user_env_ui,
            &mut log_scroll,
            &search_query,
            current_build_id,
            &history_records,
            prompt_name,
            &prompt_buffer,
        ))?;

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
                                    let r = Arc::new(Runner::new(pipeline.clone(), user_env.clone(), secrets.clone()));
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
                                    let r = Arc::new(Runner::new(pipeline.clone(), user_env.clone(), secrets.clone()));
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
                        KeyCode::Backspace => { search_query.pop(); }
                        KeyCode::Char(c) => { search_query.push(c); }
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Char('/') => {
                            is_searching = true;
                            search_query.clear();
                        }
                        KeyCode::Esc => search_query.clear(),
                        KeyCode::Char('1') => current_view = AppView::Dashboard,
                        KeyCode::Char('2') => current_view = AppView::History,
                        KeyCode::Char('3') => current_view = AppView::Settings,
                        KeyCode::Char('4') => current_view = AppView::EnvVars,
                        KeyCode::Char('r') => {
                            if let Some(r) = &runner {
                                let r_clone = r.clone();
                                tokio::spawn(async move {
                                    r_clone.reset().await;
                                    r_clone.run().await;
                                });
                            }
                        }
                        KeyCode::Up | KeyCode::Char('k') if current_view == AppView::Dashboard => {
                            if selected_job > 0 {
                                selected_job -= 1;
                                log_scroll = 0;
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') if current_view == AppView::Dashboard => {
                            if !states.is_empty() && selected_job < states.len() - 1 {
                                selected_job += 1;
                                log_scroll = 0;
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
