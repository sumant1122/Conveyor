# Conveyor 🏗️

A lightweight, local-first CI/CD tool written in Rust with a modern, real-time Terminal User Interface (TUI).

## Features
- **Parallel Execution**: Run up to `n` jobs concurrently (defaulting to 4) using `tokio`.
- **Log Search & Filtering**: Quickly find errors or specific output with real-time log filtering (`/`).
- **Dependency Tracking**: Define complex job execution order with the `needs` keyword.
- **Responsive TUI**: A spacious, modern interface that adapts to your terminal size.
- **Environment Variables**: Support for pipeline-level, job-specific, and local `env.yaml` variables.
- **Git Integration**: Live display of current branch and latest commit info in the header.
- **Post-Execution Hooks**: Custom `on_success` and `on_failure` shell commands.
- **Cross-Platform**: Automatically selects the correct shell (`cmd` for Windows, `sh` for Linux/macOS).

## Installation
Ensure you have the Rust toolchain installed.

```bash
git clone https://github.com/yourusername/conveyor.git
cd conveyor
cargo build --release
```

## Usage
1. Create a `pipeline.yaml` in your project root.
2. (Optional) Create an `env.yaml` for local/secret environment variables.
3. Run Conveyor:
   ```bash
   cargo run
   ```

### Navigation & Controls
- **'1' / '2' / '3'**: Switch between **Dashboard**, **Pipeline Config**, and **Env Variables**.
- **Up/Down Arrows**: Select a job in the Dashboard to view its logs.
- **'/'**: Enter **Search Mode** to filter logs in real-time.
- **'Esc'**: Exit search mode or clear the current search query.
- **'PgUp' / 'PgDn'**: Scroll through logs.
- **'Home' / 'End'**: Jump to the start or end of the logs.
- **'q'**: Quit the application.

## Pipeline Configuration (`pipeline.yaml`)
Example `pipeline.yaml` with concurrency and dependencies:

```yaml
name: Conveyor Build
concurrency: 4  # Max number of parallel jobs
on_failure: "echo 'Build failed!'"
on_success: "echo 'Build successful!'"

jobs:
  - name: Lint
    steps:
      - name: Check
        command: cargo clippy
        
  - name: Build
    steps:
      - name: Compile
        command: cargo build
        
  - name: Test
    needs: ["Build"] # Only runs after 'Build' succeeds
    env:
      RUST_BACKTRACE: "1"
    steps:
      - name: Unit Tests
        command: cargo test
```

## Local Environment Variables (`env.yaml`)
Store sensitive or machine-specific variables in an `env.yaml` file. These are automatically merged into all jobs.

Example `env.yaml`:
```yaml
API_KEY: "your-secret-key"
DEBUG: "true"
```

## License
MIT
