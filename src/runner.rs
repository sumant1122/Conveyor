use std::collections::HashSet;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex;
use std::sync::Arc;
use crate::pipeline::{Pipeline, Job, Step};
use serde::{Serialize, Deserialize};
use std::path::PathBuf;
use chrono::{DateTime, Local};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum JobStatus {
    Pending,
    Running,
    Success,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobState {
    pub name: String,
    pub stage_name: String,
    pub status: JobStatus,
    pub logs: Vec<String>,
    #[serde(skip)]
    pub start_time: Option<Instant>,
    pub duration: Option<std::time::Duration>,
    pub start_timestamp: Option<DateTime<Local>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildRecord {
    pub id: u32,
    pub pipeline_name: String,
    pub timestamp: DateTime<Local>,
    pub status: JobStatus,
    pub jobs: Vec<JobState>,
}

pub struct HistoryManager {
    pub root: PathBuf,
}

impl HistoryManager {
    pub fn new() -> Self {
        let root = PathBuf::from("history");
        if !root.exists() {
            let _ = std::fs::create_dir_all(&root);
        }
        Self { root }
    }

    pub fn save_build(&self, record: &BuildRecord) -> anyhow::Result<()> {
        let build_dir = self.root.join(format!("build_{}", record.id));
        std::fs::create_dir_all(&build_dir)?;
        let json = serde_json::to_string_pretty(record)?;
        std::fs::write(build_dir.join("record.json"), json)?;
        Ok(())
    }

    pub fn get_next_id(&self) -> u32 {
        let mut max_id = 0;
        if let Ok(entries) = std::fs::read_dir(&self.root) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.starts_with("build_") {
                        if let Ok(id) = name[6..].parse::<u32>() {
                            max_id = max_id.max(id);
                        }
                    }
                }
            }
        }
        max_id + 1
    }

    pub fn load_history(&self) -> Vec<BuildRecord> {
        let mut history = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&self.root) {
            for entry in entries.flatten() {
                let path = entry.path().join("record.json");
                if path.exists() {
                    if let Ok(content) = std::fs::read_to_string(path) {
                        if let Ok(record) = serde_json::from_str::<BuildRecord>(&content) {
                            history.push(record);
                        }
                    }
                }
            }
        }
        history.sort_by(|a, b| b.id.cmp(&a.id));
        history
    }
}

use ratatui::prelude::Stylize;
use std::time::Instant;

impl JobState {
    pub fn elapsed(&self) -> String {
        let duration = match self.status {
            JobStatus::Running => self.start_time.map(|s| s.elapsed()),
            _ => self.duration,
        };

        match duration {
            Some(d) => format!("{}.{:01}s", d.as_secs(), d.subsec_millis() / 100),
            None => "--".to_string(),
        }
    }
}

pub struct Runner {
    pub pipeline: Arc<Mutex<Pipeline>>,
    pub states: Arc<Mutex<Vec<JobState>>>,
    pub workspace: Arc<Mutex<Option<PathBuf>>>,
    pub user_env: std::collections::HashMap<String, String>,
    pub secrets: std::collections::HashMap<String, String>,
    pub mask_values: Vec<String>,
    pub history: HistoryManager,
    pub build_id: u32,
}

impl Runner {
    pub fn new(pipeline: Pipeline, user_env: std::collections::HashMap<String, String>, secrets: std::collections::HashMap<String, String>) -> Self {
        let history = HistoryManager::new();
        let build_id = history.get_next_id();
        let mut states = Vec::new();

        // Add a "Clone" job if a repository is specified
        if pipeline.repository.is_some() {
            states.push(JobState {
                name: "Clone Workspace".to_string(),
                stage_name: "Preparation".to_string(),
                status: JobStatus::Pending,
                logs: Vec::new(),
                start_time: None,
                duration: None,
                start_timestamp: None,
            });
        }

        if let Some(stages) = &pipeline.stages {
            for stage in stages {
                for j in &stage.jobs {
                    states.push(JobState {
                        name: j.name.clone(),
                        stage_name: stage.name.clone(),
                        status: JobStatus::Pending,
                        logs: Vec::new(),
                        start_time: None,
                        duration: None,
                        start_timestamp: None,
                    });
                }
            }
        } else if let Some(jobs) = &pipeline.jobs {
            for j in jobs {
                states.push(JobState {
                    name: j.name.clone(),
                    stage_name: "Jobs".to_string(),
                    status: JobStatus::Pending,
                    logs: Vec::new(),
                    start_time: None,
                    duration: None,
                    start_timestamp: None,
                });
            }
        }

        let mut mask_values = secrets.values().cloned().collect::<Vec<_>>();
        mask_values.retain(|v| !v.is_empty() && v.len() > 3);

        Self {
            pipeline: Arc::new(Mutex::new(pipeline)),
            states: Arc::new(Mutex::new(states)),
            workspace: Arc::new(Mutex::new(None)),
            user_env,
            secrets,
            mask_values,
            history,
            build_id,
        }
    }

    pub fn clone_for_spawn(&self) -> Self {
        Self {
            pipeline: self.pipeline.clone(),
            states: self.states.clone(),
            workspace: self.workspace.clone(),
            user_env: self.user_env.clone(),
            secrets: self.secrets.clone(),
            mask_values: self.mask_values.clone(),
            history: HistoryManager { root: self.history.root.clone() },
            build_id: self.build_id,
        }
    }

    fn mask_line(&self, line: String) -> String {
        let mut result = line;
        for mask in &self.mask_values {
            result = result.replace(mask, "****");
        }
        result
    }

    pub async fn run(&self) {
        self.internal_run().await;
    }

    pub async fn reset(&self) {
        let mut states = self.states.lock().await;
        let p = self.pipeline.lock().await;
        
        states.clear();
        if p.repository.is_some() {
            states.push(JobState {
                name: "Clone Workspace".to_string(),
                stage_name: "Preparation".to_string(),
                status: JobStatus::Pending,
                logs: Vec::new(),
                start_time: None,
                duration: None,
                start_timestamp: None,
            });
        }

        if let Some(stages) = &p.stages {
            for stage in stages {
                for j in &stage.jobs {
                    states.push(JobState {
                        name: j.name.clone(),
                        stage_name: stage.name.clone(),
                        status: JobStatus::Pending,
                        logs: Vec::new(),
                        start_time: None,
                        duration: None,
                        start_timestamp: None,
                    });
                }
            }
        } else if let Some(jobs) = &p.jobs {
            for j in jobs {
                states.push(JobState {
                    name: j.name.clone(),
                    stage_name: "Jobs".to_string(),
                    status: JobStatus::Pending,
                    logs: Vec::new(),
                    start_time: None,
                    duration: None,
                    start_timestamp: None,
                });
            }
        }
    }

    pub async fn internal_run(&self) {
        // 1. Prepare Workspace
        let (repo_opt, branch_opt, on_failure_opt) = {
            let p = self.pipeline.lock().await;
            (p.repository.clone(), p.branch.clone(), p.on_failure.clone())
        };

        if let Some(repo) = repo_opt {
            let branch = branch_opt.unwrap_or_else(|| "main".to_string());
            if !self.clone_workspace(&repo, &branch).await {
                let _ = self.run_hook(&on_failure_opt, 0).await;
                return;
            }
            
            // AFTER CLONE: Dynamically load pipeline.yaml from the workspace
            let ws_opt = {
                let ws = self.workspace.lock().await;
                ws.clone()
            };

            if let Some(ws) = ws_opt {
                let yaml_path = ws.join("pipeline.yaml");
                if yaml_path.exists() {
                    if let Ok(content) = tokio::fs::read_to_string(&yaml_path).await {
                        if let Ok(mut new_pipeline) = Pipeline::from_yaml(&content) {
                            let mut p = self.pipeline.lock().await;
                            // Preserve repo/branch info
                            new_pipeline.repository = p.repository.clone();
                            new_pipeline.branch = p.branch.clone();
                            *p = new_pipeline;
                            
                            // Re-initialize states with the new jobs
                            let mut states = self.states.lock().await;
                            // Keep the first state (Clone Workspace) if it was successful
                            let clone_state = states[0].clone();
                            states.clear();
                            states.push(clone_state);
                            
                            if let Some(stages) = &p.stages {
                                for stage in stages {
                                    for j in &stage.jobs {
                                        if j.name == "Clone Workspace" { continue; }
                                        states.push(JobState {
                                            name: j.name.clone(),
                                            stage_name: stage.name.clone(),
                                            status: JobStatus::Pending,
                                            logs: Vec::new(),
                                            start_time: None,
                                            duration: None,
                                            start_timestamp: None,
                                        });
                                    }
                                }
                            } else if let Some(jobs) = &p.jobs {
                                for j in jobs {
                                    if j.name == "Clone Workspace" { continue; }
                                    states.push(JobState {
                                        name: j.name.clone(),
                                        stage_name: "Jobs".to_string(),
                                        status: JobStatus::Pending,
                                        logs: Vec::new(),
                                        start_time: None,
                                        duration: None,
                                        start_timestamp: None,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        let (total_jobs, has_repo, concurrency_limit) = {
            let p = self.pipeline.lock().await;
            (p.get_all_jobs().len(), p.repository.is_some(), p.concurrency.unwrap_or(4))
        };
        
        let mut completed_jobs = HashSet::new();
        if has_repo {
            completed_jobs.insert("Clone Workspace".to_string());
        }
        let mut running_jobs = HashSet::new();

        // 2. Run Jobs
        while completed_jobs.len() < (total_jobs + if has_repo { 1 } else { 0 }) {
            // Get jobs that ARE NOT running, NOT completed, and have their requirements met
            let jobs_to_run: Vec<(usize, Job)> = {
                let p = self.pipeline.lock().await;
                let all_jobs = p.get_all_jobs();
                all_jobs.into_iter().enumerate()
                    .filter(|(i, j)| {
                        if running_jobs.contains(&j.name) || completed_jobs.contains(&j.name) {
                            return false;
                        }

                        if let Some(needs) = &j.needs {
                             needs.iter().all(|n| completed_jobs.contains(n))
                        } else if j.parallel == Some(true) {
                             true
                        } else if *i > 0 {
                             let prev_job_name = &p.get_all_jobs()[*i-1].name;
                             completed_jobs.contains(prev_job_name)
                        } else {
                             true
                        }
                    })
                    .map(|(i, j)| (i + if has_repo { 1 } else { 0 }, j))
                    .collect()
            };

            for (index, job) in jobs_to_run {
                if running_jobs.len() >= concurrency_limit {
                    break;
                }
                
                running_jobs.insert(job.name.clone());
                let self_clone = Arc::new(self.clone_for_spawn());
                tokio::spawn(async move {
                    self_clone.run_job(index, job).await;
                });
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            
            let states = self.states.lock().await;
            for state in states.iter() {
                if (state.status == JobStatus::Success || state.status == JobStatus::Failed) 
                    && !completed_jobs.contains(&state.name) {
                    completed_jobs.insert(state.name.clone());
                    running_jobs.remove(&state.name);
                }
            }
        }

        // 3. Post-pipeline hooks
        let (all_success, on_success_opt, on_failure_opt, last_job_index) = {
            let states = self.states.lock().await;
            let p = self.pipeline.lock().await;
            (
                states.iter().all(|s| s.status == JobStatus::Success), 
                p.on_success.clone(), 
                p.on_failure.clone(),
                states.len().saturating_sub(1)
            )
        };

        if all_success {
            let _ = self.run_hook(&on_success_opt, last_job_index).await;
        } else {
            let _ = self.run_hook(&on_failure_opt, last_job_index).await;
        }

        let _ = self.save_to_history().await;
    }
    async fn run_hook(&self, cmd_opt: &Option<String>, last_job_index: usize) -> bool {
        let cmd = match cmd_opt {
            Some(c) => c,
            None => return true,
        };

        {
            let mut states = self.states.lock().await;
            if let Some(state) = states.get_mut(last_job_index) {
                state.logs.push("".to_string());
                state.logs.push("━".repeat(40).dim().to_string());
                state.logs.push(format!("⟫ Executing Pipeline Hook: {}", cmd).bold().blue().to_string());
            }
        }

        let (shell, flag) = if cfg!(target_os = "windows") { ("cmd", "/C") } else { ("sh", "-c") };
        let mut command = Command::new(shell);
        command.args([flag, cmd]);
        let ws_opt = {
            let ws = self.workspace.lock().await;
            ws.clone()
        };
        if let Some(ws) = ws_opt {
            command.current_dir(ws);
        }
        
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = match command.spawn() {
            Ok(c) => c,
            Err(_) => return false,
        };

        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        
        let out_h = tokio::spawn({
            let _states_clone = self.states.clone();
            let self_clone = self.clone_for_spawn();
            async move {
                let mut reader = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let mut states = _states_clone.lock().await;
                    if let Some(state) = states.get_mut(last_job_index) {
                        state.logs.push(self_clone.mask_line(line));
                    }
                }
            }
        });

        let err_h = tokio::spawn({
            let _states_clone = self.states.clone();
            let self_clone = self.clone_for_spawn();
            async move {
                let mut reader = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let mut states = _states_clone.lock().await;
                    if let Some(state) = states.get_mut(last_job_index) {
                        state.logs.push(self_clone.mask_line(line));
                    }
                }
            }
        });

        let success = child.wait().await.map(|s| s.success()).unwrap_or(false);
        let _ = tokio::join!(out_h, err_h);
        
        {
            let mut states = self.states.lock().await;
            if let Some(state) = states.get_mut(last_job_index) {
                state.logs.push(format!("⟫ Hook finished with success: {}", success).bold().dim().to_string());
            }
        }
        
        success
    }

    pub async fn save_to_history(&self) -> anyhow::Result<()> {
        let (pipeline_name, states) = {
            let p = self.pipeline.lock().await;
            let s = self.states.lock().await;
            (p.name.clone(), s.clone())
        };

        let status = if states.iter().all(|s| s.status == JobStatus::Success) {
            JobStatus::Success
        } else {
            JobStatus::Failed
        };

        let record = BuildRecord {
            id: self.build_id,
            pipeline_name,
            timestamp: Local::now(),
            status,
            jobs: states,
        };

        self.history.save_build(&record)
    }

    async fn run_job(&self, index: usize, job: Job) {
        let start = Instant::now();
        {
            let mut states = self.states.lock().await;
            states[index].status = JobStatus::Running;
            states[index].start_time = Some(start);
            states[index].start_timestamp = Some(Local::now());
            states[index].logs.push(format!("Starting job: {}", job.name));
        }

        let mut success = true;
        for step in &job.steps {
            if !self.run_step(index, step.clone(), &job).await {
                success = false;
                break;
            }
        }

        let mut states = self.states.lock().await;
        states[index].status = if success { JobStatus::Success } else { JobStatus::Failed };
        states[index].duration = Some(start.elapsed());
    }

    async fn run_step(&self, index: usize, step: Step, job: &Job) -> bool {
        {
            let mut states = self.states.lock().await;
            states[index].logs.push(format!(">> Running step: {}", step.name).bold().cyan().to_string());
        }
        let (shell, flag) = if cfg!(target_os = "windows") { ("cmd", "/C") } else { ("sh", "-c") };
        let mut cmd = Command::new(shell);
        cmd.args([flag, &step.command])
           .stdout(Stdio::piped())
           .stderr(Stdio::piped());

        let ws_opt = {
            let ws = self.workspace.lock().await;
            ws.clone()
        };
        if let Some(ws) = ws_opt {
            cmd.current_dir(ws);
        }

        {
            let p = self.pipeline.lock().await;
            if let Some(env) = &p.env { cmd.envs(env); }
        }
        if let Some(env) = &job.env { cmd.envs(env); }
        cmd.envs(&self.user_env);
        cmd.envs(&self.secrets);

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let mut states = self.states.lock().await;
                states[index].logs.push(format!("Failed to spawn command: {}", e));
                return false;
            }
        };

        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        
        let out_h = tokio::spawn({
            let _states_clone = self.states.clone();
            let self_clone = self.clone_for_spawn();
            async move {
                let mut reader = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let mut states = _states_clone.lock().await;
                    states[index].logs.push(self_clone.mask_line(line));
                }
            }
        });

        let err_h = tokio::spawn({
            let _states_clone = self.states.clone();
            let self_clone = self.clone_for_spawn();
            async move {
                let mut reader = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let mut states = _states_clone.lock().await;
                    states[index].logs.push(self_clone.mask_line(line));
                }
            }
        });

        let status = child.wait().await;
        let success = status.as_ref().map(|s| s.success()).unwrap_or(false);
        let _ = tokio::join!(out_h, err_h);

        if !success {
            let mut states = self.states.lock().await;
            if let Ok(s) = status {
                if let Some(code) = s.code() {
                    if cfg!(target_os = "windows") && code == 9009 {
                        states[index].logs.push("Error: Command not found (9009). Ensure 'uv' or 'python' is in your PATH.".to_string());
                    } else {
                        states[index].logs.push(format!("Step failed with exit code: {}", code));
                    }
                } else {
                    states[index].logs.push("Step failed (terminated by signal).".to_string());
                }
            } else {
                states[index].logs.push("Step failed (unknown error).".to_string());
            }
        }
        success
    }

    async fn clone_workspace(&self, repo: &str, branch: &str) -> bool {
        {
            let mut states = self.states.lock().await;
            states[0].status = JobStatus::Running;
            states[0].logs.push(format!("Preparing workspace for {}...", repo));
        }

        // If repo is a local path that exists, just use it
        let repo_path = std::path::Path::new(repo);
        if repo_path.exists() && repo_path.is_dir() {
            let mut states = self.states.lock().await;
            states[0].logs.push(format!("Using local directory: {}", repo));
            states[0].status = JobStatus::Success;
            let mut ws = self.workspace.lock().await;
            *ws = Some(repo_path.to_path_buf());
            return true;
        }

        let workspace_path = PathBuf::from("target/workspace");
        if workspace_path.exists() {
            if let Err(e) = tokio::fs::remove_dir_all(&workspace_path).await {
                let mut states = self.states.lock().await;
                states[0].logs.push(format!("Warning: Could not clear workspace: {}. Try closing open files in target/workspace.", e));
            }
        }
        
        // Ensure target directory exists
        let _ = tokio::fs::create_dir_all("target").await;

        let mut child = match Command::new("git")
            .args(["clone", "--depth", "1", "--branch", branch, repo, "target/workspace"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn() {
                Ok(c) => c,
                Err(e) => {
                    let mut states = self.states.lock().await;
                    states[0].logs.push(format!("Failed to start git clone: {}. Is git installed?", e));
                    states[0].status = JobStatus::Failed;
                    return false;
                }
            };

        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        
        let out_handle = tokio::spawn({
            let _states_clone = self.states.clone();
            let self_clone = self.clone_for_spawn();
            async move {
                let mut reader = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let mut states = _states_clone.lock().await;
                    states[0].logs.push(self_clone.mask_line(line));
                }
            }
        });

        let err_handle = tokio::spawn({
            let _states_clone = self.states.clone();
            let self_clone = self.clone_for_spawn();
            async move {
                let mut reader = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let mut states = _states_clone.lock().await;
                    states[0].logs.push(self_clone.mask_line(line));
                }
            }
        });

        let success = child.wait().await.map(|s| s.success()).unwrap_or(false);
        let _ = tokio::join!(out_handle, err_handle);

        let mut states = self.states.lock().await;
        if success {
            states[0].status = JobStatus::Success;
            let mut ws = self.workspace.lock().await;
            *ws = Some(workspace_path);
            true
        } else {
            states[0].status = JobStatus::Failed;
            false
        }
    }
}
