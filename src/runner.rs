use std::collections::HashSet;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex;
use std::sync::Arc;
use crate::pipeline::{Pipeline, Job, Step};
use serde::{Serialize, Deserialize};

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
}

impl Runner {
    pub fn new(pipeline: Pipeline) -> Self {
        let states = pipeline.jobs.iter().map(|j| JobState {
            name: j.name.clone(),
            status: JobStatus::Pending,
            logs: Vec::new(),
        }).collect();

        Self {
            pipeline,
            states: Arc::new(Mutex::new(states)),
        }
    }

    pub async fn run(self: Arc<Self>) {
        let mut completed_jobs = HashSet::new();
        let mut running_jobs = HashSet::new();
        let total_jobs = self.pipeline.jobs.len();

        while completed_jobs.len() < total_jobs {
            let mut launched_any = false;

            // Find jobs to run
            let jobs_to_run: Vec<(usize, Job)> = self.pipeline.jobs.iter().enumerate()
                .filter(|(_i, j)| {
                    let is_pending = {
                        // Check if it's already running or completed
                        !running_jobs.contains(&j.name) && !completed_jobs.contains(&j.name)
                    };

                    if !is_pending { return false; }

                    // Check dependencies
                    if let Some(needs) = &j.needs {
                        needs.iter().all(|n| completed_jobs.contains(n))
                    } else {
                        true
                    }
                })
                .map(|(i, j)| (i, j.clone()))
                .collect();

            for (index, job) in jobs_to_run {
                running_jobs.insert(job.name.clone());
                launched_any = true;
                let self_clone = self.clone();
                tokio::spawn(async move {
                    self_clone.run_job(index, job).await;
                });
            }

            if !launched_any && running_jobs.is_empty() {
                // Potential deadlock or all failed
                break;
            }

            // Wait for any job to finish and update completed_jobs
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

        // Run post-pipeline hooks
        let all_success = {
            let states = self.states.lock().await;
            states.iter().all(|s| s.status == JobStatus::Success)
        };

        if all_success {
            if let Some(cmd) = &self.pipeline.on_success {
                let _ = self.run_hook(cmd).await;
            }
        } else {
            if let Some(cmd) = &self.pipeline.on_failure {
                let _ = self.run_hook(cmd).await;
            }
        }
    }

    async fn run_hook(&self, cmd: &str) -> bool {
        let (shell, flag) = if cfg!(target_os = "windows") { ("cmd", "/C") } else { ("sh", "-c") };
        let mut child = match Command::new(shell).args([flag, cmd]).spawn() {
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
        {
            let mut states = self.states.lock().await;
            states[index].logs.push(format!("Executing step: {}", step.name));
        }

        let (shell, flag) = if cfg!(target_os = "windows") {
            ("cmd", "/C")
        } else {
            ("sh", "-c")
        };

        let mut cmd = Command::new(shell);
        cmd.args([flag, &step.command])
           .stdout(Stdio::piped())
           .stderr(Stdio::piped());

        // Inject env vars
        if let Some(env) = &self.pipeline.env {
            cmd.envs(env);
        }
        if let Some(env) = &job.env {
            cmd.envs(env);
        }

        let mut child = match cmd.spawn() {
                Ok(child) => child,
                Err(e) => {
                    let mut states = self.states.lock().await;
                    states[index].logs.push(format!("Failed to spawn command: {}", e));
                    return false;
                }
            };

        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();

        let states_clone = self.states.clone();
        let stdout_handle = tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let mut states = states_clone.lock().await;
                states[index].logs.push(line);
            }
        });

        let states_clone = self.states.clone();
        let stderr_handle = tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let mut states = states_clone.lock().await;
                states[index].logs.push(format!("ERR: {}", line));
            }
        });

        let status = child.wait().await;
        let _ = tokio::join!(stdout_handle, stderr_handle);

        match status {
            Ok(s) if s.success() => true,
            Ok(s) => {
                let mut states = self.states.lock().await;
                states[index].logs.push(format!("Step failed with exit code: {:?}", s.code()));
                false
            }
            Err(e) => {
                let mut states = self.states.lock().await;
                states[index].logs.push(format!("Step failed: {}", e));
                false
            }
        }
    }
}
