# Conveyor 🏗️

A lightweight, local-first CI/CD tool written in Rust with a modern, real-time Terminal User Interface (TUI).

## Features
- **Stages & Jobs**: Group related jobs into stages for better organization and a clearer overview of your pipeline's progress.
- **Build History & Persistence**: Automatically saves build results and logs to a local history, allowing you to review past performance and failures.
- **Sequential by Default**: Jobs run one-by-one in the order defined in your pipeline, ensuring a predictable flow.
- **Explicit Parallelism**: Use the `parallel: true` flag to run independent jobs concurrently.
- **Dependency Tracking (DAG)**: Fine-tune execution order with the `needs` keyword for complex dependency graphs.
- **Log Search & Filtering**: Quickly find errors or specific output with real-time log filtering (`/`).
- **Pipeline Hooks**: Define `on_success` and `on_failure` commands to run after the pipeline completes.
- **Responsive TUI**: A spacious, modern interface with OneDark colors that adapts to your terminal size.
- **Git Integration**: Live display of current branch and latest commit info in the header.
- **Credential & Secret Management**: Declare required secrets in your pipeline. Conveyor will securely prompt for missing values at startup and automatically mask them (as `****`) in all TUI logs.
- **Environment Variables**: Support for pipeline-level, job-specific, local `env.yaml` variables, and secure `secrets.yaml`.
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
- **Up/Down Arrows**: Select a job in the Dashboard to view its logs.
- **'R'**: **Retry** the current pipeline (resets states and starts fresh).
- **'/'**: Enter **Search Mode** to filter logs in real-time.
- **'Esc'**: Exit search mode or clear the current search query.
- **'PgUp' / 'PgDn'**: Scroll through logs.
- **'q'**: Quit the application.

## Pipeline Configuration (`pipeline.yaml`)
Example `pipeline.yaml` using the modern **Stages** format:

```yaml
name: Example Service
on_success: "echo 'Success! Notifications sent.'"
on_failure: "echo 'Build failed. Check history for details.'"

stages:
  - name: Build
    jobs:
      - name: Compile
        steps:
          - name: Build binary
            command: cargo build --release

  - name: Test
    jobs:
      - name: Unit Tests
        steps:
          - name: Run pytest
            command: pytest
      - name: Integration Tests
        parallel: true
        steps:
          - name: Run integration
            command: npm test

  - name: Deploy
    jobs:
      - name: Push Image
        needs: ["Unit Tests", "Integration Tests"]
        steps:
          - name: Docker Push
            command: docker push my-app:latest
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

## Roadmap / Upcoming Enhancements
To closely mirror the capabilities of professional CI systems like Jenkins, the following features are planned:

- **📦 Artifact Management**: Capture and archive build outputs (binaries, test reports) for later retrieval directly from the TUI.
- **🎛️ Input Parameters**: Support for "Build with Parameters," allowing users to select options (like environment or version) before a pipeline starts.
- **🏗️ Distributed Agents**: The ability to delegate jobs to remote machines via SSH or a custom agent protocol.
- **⏲️ Triggering System**: Background daemon mode to poll Git repositories or listen for Webhooks to trigger builds automatically.
- **🤖 Headless Mode**: Optimized CLI output mode designed for AI agents and automated scripts, facilitating programmatic parsing and interaction without the TUI.

## License
MIT
