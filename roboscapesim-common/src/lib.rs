use nalgebra::{Vector3, Quaternion};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Transform {
    pub position: Vector3<f64>,
    pub rotation: Orientation,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Orientation {
    Euler(Vector3<f64>),
    Quaternion(Quaternion<f64>)
}

impl Default for Orientation {
    fn default() -> Self {
        Self::Euler(Vector3::default())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VisualInfo {
    None,
    Color(f32,f32,f32),
    Texture(String),
    Mesh(String)
}

impl Default for VisualInfo {
    fn default() -> Self {
        Self::Color(1.0,1.0,1.0)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ObjectData {
    pub name: String,
    pub transform: Option<Transform>,
    pub visual_info: VisualInfo
}