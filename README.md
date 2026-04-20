# Conveyor 🏗️

A lightweight, local-first CI/CD tool written in Rust with a modern, real-time Terminal User Interface (TUI).

## Key Features
- **Real-time TUI**: Modern, OneDark-inspired interface for live build monitoring.
- **Artifacts & History**: Preserve build outputs and browse full logs of previous runs.
- **Dependency Tracking**: Fine-grained execution control with DAG-based job dependencies.
- **Flexible Scheduling**: Automate builds with standard Cron expressions.
- **Secure by Default**: Automatic masking of secrets in logs and interactive credential prompting.
- **Headless Mode**: Ideal for AI agents and CLI automation.

## Quick Start

### 1. Installation
```bash
git clone https://github.com/sumant1122/conveyor.git
cd conveyor
cargo build --release
```

### 2. Define your Pipeline
Create a `pipeline.yaml` in your project root:
```yaml
jobs:
  - name: Build
    command: cargo build
  - name: Test
    command: cargo test
```

### 3. Run
```bash
# Run default pipeline.yaml
cargo run

# Run a custom file
cargo run -- -f my-pipeline.yaml
```

## Pipeline Examples

### 1. Simple Build & Test
A minimal pipeline for a standard Rust project.
```yaml
jobs:
  - name: Build
    command: cargo build --release
  - name: Test
    command: cargo test
```

### 2. Parallel Multi-Language Pipeline
Run independent tasks (e.g., frontend and backend checks) concurrently.
```yaml
jobs:
  - name: Backend Test
    command: cargo test
  - name: Frontend Test
    parallel: true
    command: npm test
```

### 3. Complex DAG (Dependency Graph)
Define exactly which jobs must finish before others start.
```yaml
stages:
  - name: Setup
    jobs:
      - name: Install
        command: npm install
  - name: Quality
    jobs:
      - name: Lint
        needs: ["Install"]
        command: npm run lint
      - name: Unit Tests
        needs: ["Install"]
        command: npm test
  - name: Deploy
    jobs:
      - name: Publish
        needs: ["Lint", "Unit Tests"]
        command: npm publish
```

### 4. Artifact Collection
Capture build outputs for later retrieval.
```yaml
jobs:
  - name: Build Binary
    command: cargo build --release
    artifacts:
      - "target/release/conveyor"
  - name: Generate Docs
    command: cargo doc
    artifacts:
      - "target/doc/"
```

## Documentation
For detailed information on configuration, navigation, and advanced features, see **[documentation.md](./documentation.md)**.

- **[Pipeline Configuration](./documentation.md#pipeline-configuration-pipelineyaml)**: Stages, Jobs, DAG, and Simplified Syntax.
- **[Artifacts & Cron](./documentation.md#artifact-management)**: How to preserve build outputs and automate triggers.
- **[Secrets Management](./documentation.md#environment--secrets)**: Local variables and automatic log masking.
- **[Navigation Reference](./documentation.md#navigation--controls)**: Full list of TUI keybindings.

## License
MIT
