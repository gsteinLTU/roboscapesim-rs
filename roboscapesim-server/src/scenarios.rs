use std::{collections::BTreeMap, fs};

use log::{error, info};
use once_cell::sync::Lazy;
use serde::{Serialize, Deserialize};
use crate::room::netsblox_api::Project;

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

/// Local scenarios file
pub const DEFAULT_SCENARIOS_FILE: &str = include_str!("../default_scenarios.json");

/// Local scenarios file as a map
pub static LOCAL_SCENARIOS: Lazy<BTreeMap<String, LocalScenarioDef>> = Lazy::new(|| {
    serde_json::from_str(DEFAULT_SCENARIOS_FILE).unwrap()
});

/// The default project to load if no project is specified
pub const DEFAULT_PROJECT: &str = include_str!("../assets/scenarios/Default.xml");

/// Load a project from a given environment name, or default to sample project if None
pub async fn load_environment(environment: Option<String>) -> String {
    let environment = environment.and_then(|env| if env.trim().is_empty() { None } else { Some(env) });

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

    let mut project = match environment {
        ProjectType::RemoteProject(project_name) => {
            info!("Loading remote project {}", project_name);
            // TODO: make cloud URL configurable
            reqwest::get(format!("https://cloud.netsblox.org/projects/user/{}", project_name)).await.unwrap().json::<Project>().await.and_then(|proj| Ok(proj.to_xml())).map_err(|e| format!("failed to read file: {:?}", e))
        },
        ProjectType::LocalProject(path) => {
            info!("Loading local project {}", path);
            fs::read_to_string(path).and_then(|proj| Ok(proj)).map_err(|e| format!("failed to read file: {:?}", e))
        },
        ProjectType::ProjectXML(xml) => {
            info!("Loading project from XML");
            Ok(xml.clone())
        },
    };

    if let Err(err) = project {
        error!("Failed to load project: {:?}", err);
        project = Ok(DEFAULT_PROJECT.to_owned());
    }

    let project = project.unwrap();
    project
}
