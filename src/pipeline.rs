use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Pipeline {
    pub name: String,
    pub repository: Option<String>,
    pub branch: Option<String>,
    pub env: Option<HashMap<String, String>>,
    pub on_success: Option<String>,
    pub on_failure: Option<String>,
    pub concurrency: Option<usize>,
    pub jobs: Option<Vec<Job>>,
    pub stages: Option<Vec<Stage>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Stage {
    pub name: String,
    pub jobs: Vec<Job>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Job {
    pub name: String,
    pub env: Option<HashMap<String, String>>,
    pub needs: Option<Vec<String>>,
    pub parallel: Option<bool>,
    pub steps: Vec<Step>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Step {
    pub name: String,
    pub command: String,
}

impl Pipeline {
    pub fn from_yaml(content: &str) -> anyhow::Result<Self> {
        Ok(serde_yaml::from_str(content)?)
    }

    pub fn get_all_jobs(&self) -> Vec<Job> {
        if let Some(stages) = &self.stages {
            stages.iter().flat_map(|s| s.jobs.clone()).collect()
        } else if let Some(jobs) = &self.jobs {
            jobs.clone()
        } else {
            Vec::new()
        }
    }
}
