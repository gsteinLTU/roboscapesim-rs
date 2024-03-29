use std::{collections::BTreeMap, fs};

use log::{error, info};
use once_cell::sync::Lazy;
use roboscapesim_common::api::EnvironmentInfo;
use serde::{Serialize, Deserialize};
use crate::{room::netsblox_api::Project, api::REQWEST_CLIENT};

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Types of projects that can be loaded
pub enum ProjectType {
    // Project on NetsBlox server
    RemoteProject(String),
    // Project in default_scenarios file
    LocalProject(String),
    // Project as XML string
    ProjectXML(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Structure of definition provided by local scenarios file
pub struct LocalScenarioDef {
    pub name: String,
    pub path: String,
    pub creator: Option<String>,
    pub description: Option<String>,
    pub host: String,
}

impl Into<EnvironmentInfo> for LocalScenarioDef {
    fn into(self) -> EnvironmentInfo {
        EnvironmentInfo {
            id: self.name.clone(),
            name: self.name,
            description: self.description.unwrap_or_else(|| "".to_string()),
        }
    }
}
/// Local scenarios file
pub const DEFAULT_SCENARIOS_FILE: &str = include_str!("../default_scenarios.json");

/// Local scenarios file as a map
pub static LOCAL_SCENARIOS: Lazy<BTreeMap<String, LocalScenarioDef>> = Lazy::new(|| {
    serde_json::from_str(DEFAULT_SCENARIOS_FILE).unwrap()
});

/// The default project to load if no project is specified
pub const DEFAULT_PROJECT: &str = include_str!("../assets/scenarios/Default.xml");

/// The base URL for the NetsBlox cloud server
// TODO: make cloud URL configurable
const CLOUD_BASE: &str = "https://cloud.netsblox.org";

/// Load a project from a given environment name, or default to sample project if None
pub async fn load_environment(environment: Option<String>) -> String {
    let environment = environment.and_then(|env| if env.trim().is_empty() { None } else { Some(env) });

    info!("Request to load environment {:?}", environment);
    
    // First, check if environment is a project ID
    let environment: ProjectType = if let Some(env) = &environment {
        let env = env.to_owned();
        if env.contains('/') {
            // Assume it's a project ID
            ProjectType::RemoteProject(env)
        } else {
            // Check if it's a local scenario
            let env = env.to_lowercase();
            if LOCAL_SCENARIOS.contains_key(&env) {
                if let Some(scenario) = LOCAL_SCENARIOS.get(&env) {
                    if scenario.host == "local" {
                        ProjectType::LocalProject(LOCAL_SCENARIOS.get(&env).unwrap().path.to_owned())
                    } else {
                        ProjectType::RemoteProject(LOCAL_SCENARIOS.get(&env).unwrap().path.to_owned())
                    }
                } else {
                    // Default to sample project
                    ProjectType::ProjectXML(DEFAULT_PROJECT.to_owned())
                }
            } else {
                // Default to sample project
                ProjectType::ProjectXML(DEFAULT_PROJECT.to_owned())
            }
        }
    } else {
        // Default to sample project
        ProjectType::ProjectXML(DEFAULT_PROJECT.to_owned())
    };

    let mut project = get_project(&environment).await;

    if let Err(err) = project {
        error!("Failed to load project: {:?}", err);

        info!("Retrying");
        project = get_project(&environment).await;

        if let Err(err) = project {
            error!("Failed to load project: {:?}", err);
            info!("Loading default project");
            project = Ok(DEFAULT_PROJECT.to_owned());
        }
    }
 
    project.unwrap()
}

pub async fn get_project(project: &ProjectType) -> Result<String, String> {
    match project {
        ProjectType::RemoteProject(project_name) => {
            info!("Loading remote project {}", project_name);
            let request = REQWEST_CLIENT.get(format!("{}/projects/user/{}", CLOUD_BASE, project_name)).send().await;
            if request.is_err() {
                Err(format!("failed to load project: {:?}", request.unwrap_err()))
            } else {
                request.unwrap().json::<Project>().await.and_then(|proj| Ok(proj.to_xml())).map_err(|e| format!("failed to read file: {:?}", e))
            }
        },
        ProjectType::LocalProject(path) => {
            info!("Loading local project {}", path);
            fs::read_to_string(path).and_then(|proj| Ok(proj)).map_err(|e| format!("failed to read file: {:?}", e))
        },
        ProjectType::ProjectXML(xml) => {
            info!("Loading project from XML");
            Ok(xml.clone())
        },
    }
}
