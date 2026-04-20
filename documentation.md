# Conveyor Documentation 🏗️

This document provides a comprehensive guide to configuring and using Conveyor, a lightweight, local-first CI/CD runner.

---

## Table of Contents
1. [Navigation & Controls](#navigation--controls)
2. [Pipeline Configuration (pipeline.yaml)](#pipeline-configuration-pipelineyaml)
    - [Stages & Jobs](#stages--jobs)
    - [Simplified Syntax](#simplified-syntax)
    - [Dependency Tracking (DAG)](#dependency-tracking-dag)
    - [Artifact Management](#artifact-management)
    - [Cron Scheduling](#cron-scheduling)
3. [Environment & Secrets](#environment--secrets)
    - [env.yaml](#envyaml)
    - [secrets.yaml](#secretsyaml)
4. [Headless Mode](#headless-mode)
5. [Remote Repositories](#remote-repositories)

---

## Navigation & Controls

Conveyor provides a real-time TUI for managing your builds.

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

---

## Pipeline Configuration (pipeline.yaml)

Conveyor searches for `pipeline.yaml` in your project root.

### Stages & Jobs
Stages allow you to group related jobs. Jobs within a stage can run in parallel, while stages themselves generally progress sequentially.

### Simplified Syntax
You can omit verbose fields for simple tasks:
- **Shorthand Step**: A string instead of an object if only a command is needed.
- **Shorthand Job**: Use `command` directly on the job if it only has one step.

```yaml
# Optional: Defaults to 'Conveyor Build'
name: My Pipeline 
schedule: "0 */1 * * * *" # Cron: sec min hour day month dow

stages:
  - name: Build
    jobs:
      - name: Compile
        command: cargo build --release # Shorthand job
        artifacts:
          - "target/release/app"

  - name: Test
    jobs:
      - name: Unit Tests
        steps:
          - cargo test # Shorthand step
```

### Dependency Tracking (DAG)
Use `needs` to define a Directed Acyclic Graph of dependencies.
```yaml
jobs:
  - name: Deploy
    needs: ["Unit Tests", "Integration Tests"]
    command: ./deploy.sh
```

### Artifact Management
Specify files or directories to preserve after a successful job. They are stored in `history/build_{id}/artifacts/`.
```yaml
artifacts:
  - "target/release/binary"
  - "docs/html/"
```

### Cron Scheduling
Standard 6-field cron expressions are supported for automated runs.
`sec min hour day month dow`

---

## Environment & Secrets

### env.yaml
Store non-sensitive local variables here.
```yaml
DEBUG: "true"
API_URL: "http://localhost:8080"
```

### secrets.yaml
Store sensitive credentials here. Values are **automatically masked** in all TUI logs.
```yaml
DATABASE_PASSWORD: "super-secret-password"
```

To require a secret, declare it in `pipeline.yaml`. Conveyor will prompt for it at startup if missing.
```yaml
secrets:
  - DATABASE_PASSWORD
```

---

## Headless Mode

For use in CI environments or by AI agents. Logs stream to `stdout` and the process exits with `0` (success) or `1` (failure).

```bash
cargo run -- --headless
```

---

## Remote Repositories

Run a pipeline directly from a Git URL. Conveyor clones it into a unique temporary workspace.

```bash
cargo run -- https://github.com/user/repo.git [optional-branch]
```
