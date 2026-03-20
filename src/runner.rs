use std::collections::HashSet;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex;
use std::sync::Arc;
use crate::pipeline::{Pipeline, Job, Step};
use serde::{Serialize, Deserialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum JobStatus {
    Pending,
    Running,
    Success,
    Failed,
}

#[derive(Debug, Clone)]
pub struct JobState {
    pub name: String,
    pub status: JobStatus,
    pub logs: Vec<String>,
}

pub struct Runner {
    pub pipeline: Pipeline,
    pub states: Arc<Mutex<Vec<JobState>>>,
    pub workspace: Option<PathBuf>,
    pub user_env: std::collections::HashMap<String, String>,
}

impl Runner {
    pub fn new(pipeline: Pipeline, user_env: std::collections::HashMap<String, String>) -> Self {
        let mut states = Vec::new();
        
        // Add a "Clone" job if a repository is specified
        if pipeline.repository.is_some() {
            states.push(JobState {
                name: "Clone Workspace".to_string(),
                status: JobStatus::Pending,
                logs: Vec::new(),
            });
        }

        for j in &pipeline.jobs {
            states.push(JobState {
                name: j.name.clone(),
                status: JobStatus::Pending,
                logs: Vec::new(),
            });
        }

        Self {
            pipeline,
            states: Arc::new(Mutex::new(states)),
            workspace: None,
            user_env,
        }
    }

    pub async fn run(mut self) {
        // 1. Prepare Workspace
        let repo_opt = self.pipeline.repository.clone();
        if let Some(repo) = repo_opt {
            let branch = self.pipeline.branch.clone().unwrap_or_else(|| "main".to_string());
            if !self.clone_workspace(&repo, &branch).await {
                let _ = self.run_hook(&self.pipeline.on_failure).await;
                return;
            }
            
            // AFTER CLONE: Dynamically load pipeline.yaml from the workspace
            if let Some(ws) = &self.workspace {
                let yaml_path = ws.join("pipeline.yaml");
                if yaml_path.exists() {
                    if let Ok(content) = tokio::fs::read_to_string(&yaml_path).await {
                        if let Ok(mut new_pipeline) = Pipeline::from_yaml(&content) {
                            // Preserve repo/branch info
                            new_pipeline.repository = self.pipeline.repository.clone();
                            new_pipeline.branch = self.pipeline.branch.clone();
                            self.pipeline = new_pipeline;
                            
                            // Re-initialize states with the new jobs
                            let mut states = self.states.lock().await;
                            // Keep the first state (Clone Workspace) if it was successful
                            let clone_state = states[0].clone();
                            states.clear();
                            states.push(clone_state);
                            
                            for j in &self.pipeline.jobs {
                                states.push(JobState {
                                    name: j.name.clone(),
                                    status: JobStatus::Pending,
                                    logs: Vec::new(),
                                });
                            }
                        }
                    }
                }
            }
        }

        let total_jobs = self.pipeline.jobs.len();
        let mut completed_jobs = HashSet::new();
        if self.pipeline.repository.is_some() {
            completed_jobs.insert("Clone Workspace".to_string());
        }
        let mut running_jobs = HashSet::new();

        let arc_self = Arc::new(self);

        // 2. Run Jobs
        while completed_jobs.len() < (total_jobs + if arc_self.pipeline.repository.is_some() { 1 } else { 0 }) {
            let mut launched_any = false;

            let jobs_to_run: Vec<(usize, Job)> = arc_self.pipeline.jobs.iter().enumerate()
                .filter(|(_i, j)| {
                    if running_jobs.contains(&j.name) || completed_jobs.contains(&j.name) {
                        return false;
                    }
                    if let Some(needs) = &j.needs {
                        needs.iter().all(|n| completed_jobs.contains(n))
                    } else {
                        true
                    }
                })
                .map(|(i, j)| (i + if arc_self.pipeline.repository.is_some() { 1 } else { 0 }, j.clone()))
                .collect();

            for (index, job) in jobs_to_run {
                running_jobs.insert(job.name.clone());
                launched_any = true;
                let self_clone = arc_self.clone();
                tokio::spawn(async move {
                    self_clone.run_job(index, job).await;
                });
            }

            if !launched_any && running_jobs.is_empty() {
                break;
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            
            let states = arc_self.states.lock().await;
            for state in states.iter() {
                if (state.status == JobStatus::Success || state.status == JobStatus::Failed) 
                    && !completed_jobs.contains(&state.name) {
                    completed_jobs.insert(state.name.clone());
                    running_jobs.remove(&state.name);
                }
            }
        }

        // 3. Post-pipeline hooks
        let all_success = {
            let states = arc_self.states.lock().await;
            states.iter().all(|s| s.status == JobStatus::Success)
        };

        if all_success {
            let _ = arc_self.run_hook(&arc_self.pipeline.on_success).await;
        } else {
            let _ = arc_self.run_hook(&arc_self.pipeline.on_failure).await;
        }
    }

    async fn clone_workspace(&mut self, repo: &str, branch: &str) -> bool {
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
            self.workspace = Some(repo_path.to_path_buf());
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
        let states_clone = self.states.clone();
        
        let out_handle = tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let mut states = states_clone.lock().await;
                states[0].logs.push(line);
            }
        });

        let err_handle = tokio::spawn({
            let states_clone = self.states.clone();
            async move {
                let mut reader = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let mut states = states_clone.lock().await;
                    // Standard CLI tools often use stderr for progress info. 
                    // We'll just log the line as is.
                    states[0].logs.push(line);
                }
            }
        });

        let success = child.wait().await.map(|s| s.success()).unwrap_or(false);
        let _ = tokio::join!(out_handle, err_handle);

        let mut states = self.states.lock().await;
        if success {
            states[0].status = JobStatus::Success;
            self.workspace = Some(workspace_path);
            true
        } else {
            states[0].status = JobStatus::Failed;
            false
        }
    }

    async fn run_hook(&self, cmd_opt: &Option<String>) -> bool {
        let cmd = match cmd_opt {
            Some(c) => c,
            None => return true,
        };
        let (shell, flag) = if cfg!(target_os = "windows") { ("cmd", "/C") } else { ("sh", "-c") };
        let mut command = Command::new(shell);
        command.args([flag, cmd]);
        if let Some(ws) = &self.workspace {
            command.current_dir(ws);
        }
        let mut child = match command.spawn() {
            Ok(c) => c,
            Err(_) => return false,
        };
        child.wait().await.map(|s| s.success()).unwrap_or(false)
    }

    async fn run_job(&self, index: usize, job: Job) {
        {
            let mut states = self.states.lock().await;
            states[index].status = JobStatus::Running;
            states[index].logs.push(format!("Starting job: {}", job.name));
        }

        for step in &job.steps {
            if !self.run_step(index, step.clone(), &job).await {
                let mut states = self.states.lock().await;
                states[index].status = JobStatus::Failed;
                return;
            }
        }

        let mut states = self.states.lock().await;
        states[index].status = JobStatus::Success;
    }

    async fn run_step(&self, index: usize, step: Step, job: &Job) -> bool {
        let (shell, flag) = if cfg!(target_os = "windows") { ("cmd", "/C") } else { ("sh", "-c") };
        let mut cmd = Command::new(shell);
        cmd.args([flag, &step.command])
           .stdout(Stdio::piped())
           .stderr(Stdio::piped());

        if let Some(ws) = &self.workspace {
            cmd.current_dir(ws);
        }

        if let Some(env) = &self.pipeline.env { cmd.envs(env); }
        if let Some(env) = &job.env { cmd.envs(env); }
        cmd.envs(&self.user_env);

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
        let states_clone = self.states.clone();
        
        let out_h = tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let mut states = states_clone.lock().await;
                states[index].logs.push(line);
            }
        });

        let err_h = tokio::spawn({
            let states_clone = self.states.clone();
            async move {
                let mut reader = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let mut states = states_clone.lock().await;
                    states[index].logs.push(line);
                }
            }
        });

        let success = child.wait().await.map(|s| s.success()).unwrap_or(false);
        let _ = tokio::join!(out_h, err_h);

        if !success {
            let mut states = self.states.lock().await;
            states[index].logs.push("Step failed.".to_string());
        }
        success
    }
}
