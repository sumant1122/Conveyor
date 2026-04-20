use serde::{Deserialize, Serialize};
use std::collections::HashMap;

fn default_pipeline_name() -> String {
    "Conveyor Build".to_string()
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Pipeline {
    #[serde(default = "default_pipeline_name")]
    pub name: String,
    pub repository: Option<String>,
    pub branch: Option<String>,
    pub schedule: Option<String>,
    pub env: Option<HashMap<String, String>>,
    pub secrets: Option<Vec<String>>,
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
    pub secrets: Option<Vec<String>>,
    pub needs: Option<Vec<String>>,
    pub parallel: Option<bool>,
    pub artifacts: Option<Vec<String>>,
    #[serde(rename = "steps")]
    pub raw_steps: Option<Vec<StepConfig>>,
    pub command: Option<String>,
    #[serde(skip)]
    pub steps: Vec<Step>,
}

impl Job {
    pub fn normalize(&mut self) {
        let mut final_steps = Vec::new();

        if let Some(cmd) = &self.command {
            final_steps.push(Step {
                name: "Execute".to_string(),
                command: cmd.clone(),
                secrets: None,
            });
        } else if let Some(raw) = &self.raw_steps {
            for (i, config) in raw.iter().enumerate() {
                match config {
                    StepConfig::Simple(cmd) => {
                        final_steps.push(Step {
                            name: format!("Step {}", i + 1),
                            command: cmd.clone(),
                            secrets: None,
                        });
                    }
                    StepConfig::Detailed(detail) => {
                        final_steps.push(Step {
                            name: detail
                                .name
                                .clone()
                                .unwrap_or_else(|| format!("Step {}", i + 1)),
                            command: detail.command.clone(),
                            secrets: detail.secrets.clone(),
                        });
                    }
                }
            }
        }

        self.steps = final_steps;
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum StepConfig {
    Simple(String),
    Detailed(StepDetail),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StepDetail {
    pub name: Option<String>,
    pub secrets: Option<Vec<String>>,
    pub command: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Step {
    pub name: String,
    pub secrets: Option<Vec<String>>,
    pub command: String,
}

impl Pipeline {
    pub fn from_yaml(content: &str) -> anyhow::Result<Self> {
        let mut pipeline: Self = serde_yaml::from_str(content)?;
        pipeline.normalize();
        pipeline.validate()?;
        Ok(pipeline)
    }

    fn normalize(&mut self) {
        if let Some(stages) = &mut self.stages {
            for stage in stages {
                for job in &mut stage.jobs {
                    job.normalize();
                }
            }
        }
        if let Some(jobs) = &mut self.jobs {
            for job in jobs {
                job.normalize();
            }
        }
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        let jobs = self.get_all_jobs();
        let job_names: std::collections::HashSet<_> = jobs.iter().map(|j| &j.name).collect();

        for job in &jobs {
            if let Some(needs) = &job.needs {
                for need in needs {
                    if !job_names.contains(need) {
                        anyhow::bail!("Job '{}' depends on unknown job '{}'", job.name, need);
                    }
                    if need == &job.name {
                        anyhow::bail!("Job '{}' cannot depend on itself", job.name);
                    }
                }
            }
        }

        // Check for cycles
        let mut visited = std::collections::HashSet::new();
        let mut stack = std::collections::HashSet::new();

        fn has_cycle(
            job_name: &str,
            jobs_map: &std::collections::HashMap<&String, &Job>,
            visited: &mut std::collections::HashSet<String>,
            stack: &mut std::collections::HashSet<String>,
        ) -> bool {
            visited.insert(job_name.to_string());
            stack.insert(job_name.to_string());

            if let Some(needs) = jobs_map
                .get(&job_name.to_string())
                .and_then(|j| j.needs.as_ref())
            {
                for need in needs {
                    if !visited.contains(need) {
                        if has_cycle(need, jobs_map, visited, stack) {
                            return true;
                        }
                    } else if stack.contains(need) {
                        return true;
                    }
                }
            }

            stack.remove(job_name);
            false
        }

        let jobs_map: std::collections::HashMap<_, _> = jobs.iter().map(|j| (&j.name, j)).collect();
        for job in &jobs {
            if !visited.contains(&job.name)
                && has_cycle(&job.name, &jobs_map, &mut visited, &mut stack)
            {
                anyhow::bail!("Cyclic dependency detected in pipeline");
            }
        }

        Ok(())
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
