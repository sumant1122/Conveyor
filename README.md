# Conveyor 🏗️

A lightweight, local-first CI/CD tool written in Rust with a modern, real-time Terminal User Interface (TUI).

## Features
- **Artifact Management**: Define `artifacts` in your jobs. Conveyor will automatically collect and preserve these files (binaries, reports, etc.) in the build history.
- **Interactive History**: Browse previous builds, view their statuses, and inspect full terminal logs directly from the history tab.
- **Log Highlighting**: Real-time log search (`/`) not only filters lines but also highlights matching substrings for better visibility.
- **Cron Scheduling**: Automate your pipelines with standard cron expressions (e.g., `schedule: "0 */6 * * *"`).
- **Stages & Jobs**: Group related jobs into stages for better organization and a clearer overview of your pipeline's progress.
- **Build History & Persistence**: Automatically saves build results and logs to a local history, allowing you to review past performance and failures.
- **Sequential by Default**: Jobs run one-by-one in the order defined in your pipeline, ensuring a predictable flow.
- **Explicit Parallelism**: Use the `parallel: true` flag to run independent jobs concurrently.
- **Dependency Tracking (DAG)**: Fine-tune execution order with the `needs` keyword for complex dependency graphs.
- **Log Search & Filtering**: Quickly find errors or specific output with real-time log filtering (`/`).
- **Pipeline Hooks**: Define `on_success` and `on_failure` commands to run after the pipeline completes.
- **Responsive TUI**: A spacious, modern interface with OneDark colors that adapts to your terminal size.
- **Headless Mode**: Run pipelines in a non-interactive mode with real-time log streaming to stdout. Ideal for AI agents, CI automation, and scripts.
- **Git Integration**: Live display of current branch and latest commit info in the header.
- **Credential & Secret Management**: Declare required secrets in your pipeline. Conveyor will securely prompt for missing values at startup and automatically mask them (as `****`) in all TUI logs.
- **Environment Variables**: Support for pipeline-level, job-specific, local `env.yaml` variables, and secure `secrets.yaml`.
- **Cross-Platform**: Automatically selects the correct shell (`cmd` for Windows, `sh` for Linux/macOS).
- **Graceful Process Termination**: Safely cancels running jobs and cleans up orphaned compilation processes instantly when quitting (`Q`) or retrying (`R`).
- **Isolated Workspaces**: Remote pipelines clone into dynamically generated, globally unique directories to prevent file locking and stomping collisions.

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

### Remote Repositories
Conveyor can also run pipelines directly from a remote Git repository. It will clone the repository into a temporary workspace and automatically load the `pipeline.yaml` from its root.

```bash
# Run the default branch (usually main)
cargo run -- https://github.com/user/repo.git

# Run a specific branch
cargo run -- https://github.com/user/repo.git my-feature-branch
```

### Navigation & Controls
- **'1' / '2' / '3' / '4'**: Switch between **Dashboard**, **History**, **Pipeline Config**, and **Env Variables**.
- **Up/Down Arrows**: 
  - **Dashboard**: Select a job to view its logs.
  - **History**: Select a previous build record.
- **'Enter'**:
  - **History**: View the full details and logs of the selected historical build.
- **'Esc'**: 
  - **History Detail**: Return to the build list.
  - **Search**: Clear search query or exit search mode.
- **'R'**: **Retry** the current pipeline (resets states and starts fresh).
- **'/'**: Enter **Search Mode** to filter and **highlight** logs in real-time.
- **'PgUp' / 'PgDn'**: Scroll through logs.
- **'q'**: Quit the application.

## Pipeline Configuration (`pipeline.yaml`)
Example `pipeline.yaml` using modern features like **Stages**, **Cron**, and **Artifacts**, with **Simplified Syntax**:

```yaml
# name: Example Service (Optional: Defaults to 'Conveyor Build')
schedule: "0 */1 * * * *" # Run every minute
on_success: "echo 'Success!'"

stages:
  - name: Build
    jobs:
      - name: Compile
        command: cargo build --release
        artifacts:
          - "target/release/conveyor"

  - name: Test
    jobs:
      - name: Unit Tests
        steps:
          - cargo test # Shorthand for simple commands
        artifacts:
          - "target/debug/deps/"

  - name: Deploy
    jobs:
      - name: Push Image
        needs: ["Unit Tests"]
        command: echo "Pushing..."
```

*Note: The older flat `jobs:` format is still supported for backward compatibility.*

## Local Environment Variables (`env.yaml` & `secrets.yaml`)
Store sensitive or machine-specific variables in an `env.yaml` file. Use `secrets.yaml` for credentials; any value defined here will be **masked** in the TUI logs.

Example `env.yaml`:
```yaml
DEBUG: "true"
LOG_LEVEL: "info"
```

Example `secrets.yaml`:
```yaml
API_KEY: "my-very-secret-key"
SSH_PRIVATE_KEY: |
  -----BEGIN RSA PRIVATE KEY-----
  ...
```

To enforce secret entry, add them to your `pipeline.yaml`:
```yaml
name: My Pipeline
secrets:
  - API_KEY
  - DOCKER_PASSWORD
```
If these are not present in `secrets.yaml`, Conveyor will prompt for them securely at startup.

### Headless Mode (for AI Agents & Automation)
To run Conveyor without the TUI, use the `--headless` flag. This will stream all logs directly to stdout and exit with a proper status code (0 for success, 1 for failure).

```bash
# Run local pipeline
cargo run -- --headless

# Run remote repository
cargo run -- https://github.com/user/repo.git --headless
```

## Roadmap / Upcoming Enhancements
To closely mirror the capabilities of professional CI systems like Jenkins, the following features are planned:

- **🎛️ Input Parameters**: Support for "Build with Parameters," allowing users to select options (like environment or version) before a pipeline starts.
- **🏗️ Distributed Agents**: The ability to delegate jobs to remote machines via SSH or a custom agent protocol.
- **🐳 Container Isolation**: Run jobs inside Docker/Podman containers for clean, reproducible environments.

## License
MIT
