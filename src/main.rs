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
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::process::Command;

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

    let pipeline = Pipeline::from_yaml(&pipeline_content)?;
    let pipeline_name = pipeline.name.clone();
    let git_info = get_git_info();

    // 2. Initialize runner
    let runner = Arc::new(Runner::new(pipeline));
    let runner_states = runner.states.clone();

    // 3. Start runner in background
    let runner_clone = runner.clone();
    let _runner_handle = tokio::spawn(async move {
        runner_clone.run().await;
    });

    // 4. Set up TUI
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 5. Run TUI event loop
    let mut selected_job = 0;
    let tick_rate = Duration::from_millis(100);
    let mut last_tick = Instant::now();

    loop {
        let states = {
            let s = runner_states.lock().await;
            s.clone()
        };

        terminal.draw(|f| ui::draw(f, &states, selected_job, &git_info, &pipeline_name))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => break,
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
