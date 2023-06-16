use nalgebra::{Vector3, Quaternion};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Transform {
    pub position: Vector3<f64>,
    pub rotation: Orientation,
    pub scaling: Vector3<f64>,
}

impl Default for Transform {
    fn default() -> Self {
        Self { position: Default::default(), rotation: Default::default(), scaling: Vector3::new(1.0, 1.0, 1.0) }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum Orientation {
    Euler(Vector3<f64>),
    Quaternion(Quaternion<f64>)
}

impl Default for Orientation {
    fn default() -> Self {
        Self::Euler(Vector3::default())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ObjectData {
    pub name: String,
    pub transform: Transform,
    pub visual_info: VisualInfo
}