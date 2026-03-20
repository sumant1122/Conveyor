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
}

impl Runner {
    pub fn new(pipeline: Pipeline) -> Self {
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
        }
    }

    pub async fn run(mut self) {
        let total_jobs = self.pipeline.jobs.len();
        let mut completed_jobs = HashSet::new();
        let mut running_jobs = HashSet::new();

        // 1. Prepare Workspace
        let repo_opt = self.pipeline.repository.clone();
        if let Some(repo) = repo_opt {
            let branch = self.pipeline.branch.clone().unwrap_or_else(|| "main".to_string());
            if !self.clone_workspace(&repo, &branch).await {
                let _ = self.run_hook(&self.pipeline.on_failure).await;
                return;
            }
            completed_jobs.insert("Clone Workspace".to_string());
        }

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
            states[0].logs.push(format!("Cloning {} (branch: {})...", repo, branch));
        }

        let workspace_path = PathBuf::from("target/workspace");
        if workspace_path.exists() {
            let _ = tokio::fs::remove_dir_all(&workspace_path).await;
        }

        let mut child = match Command::new("git")
            .args(["clone", "--depth", "1", "--branch", branch, repo, "target/workspace"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn() {
                Ok(c) => c,
                Err(e) => {
                    let mut states = self.states.lock().await;
                    states[0].logs.push(format!("Failed to spawn git clone: {}", e));
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
                    states[0].logs.push(format!("ERR: {}", line));
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
                    states[index].logs.push(format!("ERR: {}", line));
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
