# Conveyor 🏗️

A lightweight, local-first CI/CD tool written in Rust with a real-time Terminal User Interface (TUI).

## Features
- **Parallel Execution**: Jobs without dependencies run concurrently using `tokio`.
- **Dependency Tracking**: Define job execution order with the `needs` keyword.
- **Environment Variables**: Support for pipeline-level, job-specific, and local `env.yaml` environment variables.
- **Git Integration**: Displays current branch and latest commit info in the TUI header.
- **Real-time Monitoring**: Live status tracking and log streaming for each job.
- **Cross-Platform**: Automatically selects the correct shell (`cmd` for Windows, `sh` for Linux/macOS).
- **Post-Execution Hooks**: Custom `on_success` and `on_failure` shell commands.

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

### Navigation
- **'1'**: Switch to **Dashboard** (Job status & logs).
- **'2'**: Switch to **Settings** (View pipeline configuration).
- **'3'**: Switch to **Env Vars** (View variables from `env.yaml`).
- **Up/Down Arrows**: Select a job in the Dashboard to view its logs.
- **'q'**: Quit the application.

## Local Environment Variables (`env.yaml`)
You can store local or sensitive environment variables in an `env.yaml` file in the project root. These variables will be available to all jobs in the pipeline.

Example `env.yaml`:
```yaml
API_KEY: "your-api-key-here"
DEBUG: "true"
DATABASE_URL: "postgres://user:password@localhost/db"
```

## `pipeline.yaml` Example
```yaml
name: Conveyor Build
env:
  PROJECT_NAME: Conveyor
on_failure: "echo 'Build failed!'"
on_success: "echo 'Build successful!'"

jobs:
  - name: Build
    steps:
      - name: Compile
        command: cargo build
  - name: Test
    needs: ["Build"]
    env:
      RUST_BACKTRACE: "1"
    steps:
      - name: Unit Tests
        command: cargo test
  - name: Lint
    steps:
      - name: Check
        command: cargo clippy
```

## Contributing
Contributions are welcome! Please open an issue or submit a pull request.

## License
MIT
