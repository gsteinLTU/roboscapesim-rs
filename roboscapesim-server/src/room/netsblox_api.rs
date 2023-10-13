use std::collections::HashMap;

use serde::{Serialize, Deserialize};

use std::time::SystemTime;

/// NetsBlox API
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ProjectId(String);

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct RoleId(String);

#[derive(Deserialize, Serialize, Clone, Debug)]
pub enum SaveState {
    Created,
    Transient,
    Broken,
    Saved,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct RoleMetadata {
    pub name: String,
    pub code: String,
    pub media: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum PublishState {
    Private,
    ApprovalDenied,
    PendingApproval,
    Public,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct RoleData {
    pub name: String,
    pub code: String,
    pub media: String,
}

impl RoleData {
    pub fn to_xml(&self) -> String {
        let name = self.name.replace('\"', "\\\"");
        format!("<role name=\"{}\">{}{}</role>", name, self.code, self.media)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: ProjectId,
    pub owner: String,
    pub name: String,
    pub updated: SystemTime,
    pub state: PublishState,
    pub collaborators: std::vec::Vec<String>,
    pub origin_time: SystemTime,
    pub save_state: SaveState,
    pub roles: HashMap<RoleId, RoleData>,
}

impl Project {
    pub fn to_xml(&self) -> String {
        let role_str: String = self
            .roles
            .values()
            .map(|role| role.to_xml())
            .collect::<Vec<_>>()
            .join(" ");
        format!(
            "<room name=\"{}\" app=\"{}\">{}</room>",
            self.name, "NetsBlox", role_str
        )
    }
}
