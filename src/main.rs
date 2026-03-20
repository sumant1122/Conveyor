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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Read pipeline.yaml
    let pipeline_content = tokio::fs::read_to_string("pipeline.yaml")
        .await
        .unwrap_or_else(|_| {
            let default = r#"name: Conveyor Build
jobs:
  - name: Build
    steps:
      - name: Compile
        command: cargo build
  - name: Test
    needs: ["Build"]
    steps:
      - name: Unit Tests
        command: cargo test
  - name: Lint
    steps:
      - name: Check
        command: cargo clippy
"#;
            std::fs::write("pipeline.yaml", default).unwrap();
            default.to_string()
        });

    let mut pipeline = Pipeline::from_yaml(&pipeline_content)?;
    
    // Check for command line repo argument
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        pipeline.repository = Some(args[1].clone());
        if args.len() > 2 {
            pipeline.branch = Some(args[2].clone());
        }
    }

    let pipeline_name = pipeline.name.clone();
    let git_info = if pipeline.repository.is_some() {
        format!("Remote: {}", pipeline.repository.as_ref().unwrap())
    } else {
        get_git_info()
    };

    // Keep a copy for UI
    let pipeline_config = pipeline.clone();

    // 2. Initialize runner
    let runner = Runner::new(pipeline);
    let runner_states = runner.states.clone();

    // 3. Start runner in background
    let _runner_handle = tokio::spawn(async move {
        runner.run().await;
    });

    // 4. Set up TUI
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 5. Run TUI event loop
    let mut selected_job = 0;
    let mut current_view = AppView::Dashboard;
    let tick_rate = Duration::from_millis(100);
    let mut last_tick = Instant::now();

    loop {
        let states = {
            let s = runner_states.lock().await;
            s.clone()
        };

        terminal.draw(|f| ui::draw(f, &states, selected_job, &git_info, &pipeline_name, &current_view, &pipeline_config))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('1') => current_view = AppView::Dashboard,
                    KeyCode::Char('2') => current_view = AppView::Settings,
                    KeyCode::Up => {
                        if selected_job > 0 {
                            selected_job -= 1;
                        }
                    }
                    KeyCode::Down => {
                        if selected_job < states.len() - 1 {
                            selected_job += 1;
                        }
                    }
                    _ => {}
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
