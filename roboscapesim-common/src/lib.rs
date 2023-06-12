use nalgebra::{Vector3, Quaternion};

pub struct Transform {
    pub position: Vector3<f64>,
    pub rotation: Orientation,
}

pub enum Orientation {
    Euler(Vector3<f64>),
    Quaternion(Quaternion<f64>)
}

pub struct ObjectData {
    pub name: String,
    pub transform: Option<Transform>
}