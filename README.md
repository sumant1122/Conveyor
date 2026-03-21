# Conveyor 🏗️

A lightweight, local-first CI/CD tool written in Rust with a modern, real-time Terminal User Interface (TUI).

## Features
- **Sequential by Default**: Jobs run one-by-one in the order defined in your pipeline, ensuring a predictable flow.
- **Explicit Parallelism**: Use the `parallel: true` flag to run independent jobs concurrently.
- **Dependency Tracking (DAG)**: Fine-tune execution order with the `needs` keyword for complex dependency graphs.
- **Log Search & Filtering**: Quickly find errors or specific output with real-time log filtering (`/`).
- **Responsive TUI**: A spacious, modern interface with OneDark colors that adapts to your terminal size.
- **Git Integration**: Live display of current branch and latest commit info in the header.
- **Environment Variables**: Support for pipeline-level, job-specific, and local `env.yaml` variables.
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
- **'q'**: Quit the application.

## Pipeline Configuration (`pipeline.yaml`)
Example `pipeline.yaml` demonstrating the execution model:

```yaml
name: Conveyor Build
concurrency: 4  # Max number of total parallel jobs allowed

jobs:
  - name: Lint
    steps:
      - name: Check
        command: cargo clippy
        
  - name: Build
    # This runs ONLY after 'Lint' succeeds (Sequential default)
    steps:
      - name: Compile
        command: cargo build
        
  - name: Unit Tests
    # This runs ONLY after 'Build' succeeds (Sequential default)
    steps:
      - name: Tests
        command: cargo test

  - name: Integration Tests
    parallel: true # Runs immediately alongside other jobs
    steps:
      - name: Run
        command: cargo test --test integration

  - name: Deploy
    needs: ["Unit Tests", "Integration Tests"] # DAG: Runs only after both are successful
    steps:
      - name: Push
        command: echo "Deploying..."
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
