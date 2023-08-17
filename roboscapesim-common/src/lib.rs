use dashmap::DashMap;
use nalgebra::{Quaternion, Vector3, vector, Point3};
use serde::{Deserialize, Serialize};

pub mod api;

pub trait Interpolatable<T> 
where Self: Sized {
    fn interpolate(&self, other: &T, t: f32) -> Self {
        self.try_interpolate(other, t).unwrap()
    }
    
    fn try_interpolate(&self, other: &T, t: f32) -> Result<Self, &'static str>;
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Transform {
    pub position: Point3<f32>,
    pub rotation: Orientation,
    pub scaling: Vector3<f32>,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: Default::default(),
            rotation: Default::default(),
            scaling: Vector3::new(1.0, 1.0, 1.0),
        }
    }
}

impl Interpolatable<Transform> for Transform {
    fn try_interpolate(&self, other: &Transform, t: f32) -> Result<Transform, &'static str> {
        // Point does not have lerp currently
        let mut lerped_pos = self.position * (1.0 - t);
        lerped_pos.x += other.position.x * t;
        lerped_pos.y += other.position.y * t;
        lerped_pos.z += other.position.z * t;
        
        let rot = self.rotation.try_interpolate(&other.rotation, t);

        if let Err(e) = rot {
            return Err(e);
        }

        Ok(Transform { position: lerped_pos, rotation: rot.unwrap(), scaling: self.scaling.lerp(&other.scaling, t) })
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum Orientation {
    Euler(Vector3<f32>),
    Quaternion(Quaternion<f32>),
}

impl Interpolatable<Orientation> for Orientation {
    fn try_interpolate(&self, other: &Orientation, t: f32) -> Result<Orientation, &'static str> {
        match self {
            Orientation::Euler(e) => {
                if let Orientation::Euler(o) = other {
                    Ok(Orientation::Euler(e.lerp(&o, t)))
                } else {
                    Err("Interpolation between Euler and quaternion Orientations not supported")                
                }
            },
            Orientation::Quaternion(q) => {
                if let Orientation::Quaternion(q2) = other {
                    Ok(Orientation::Quaternion(q.lerp(&q2, t)))
                } else {
                    Err("Interpolation between Euler and quaternion Orientations not supported")                    
                }
            },
        }
    }
}

impl Default for Orientation {
    fn default() -> Self {
        Self::Euler(Vector3::default())
    }
}

impl From<(f32, f32, f32)> for Orientation {
    fn from(value: (f32, f32, f32)) -> Self {
        Self::Euler(vector![value.0, value.1, value.2])
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum VisualInfo {
    None,
    Color(f32, f32, f32),
    Texture(String),
    Mesh(String),
}

impl Default for VisualInfo {
    fn default() -> Self {
        Self::Color(1.0, 1.0, 1.0)
    }
}

/// Generic data about an object to be sent to the client
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ObjectData {
    pub name: String,
    pub transform: Transform,
    pub visual_info: Option<VisualInfo>,
    /// If true, the object should be assumed to not move through physics
    pub is_kinematic: bool,
    /// If true, the object has been modified since last send
    pub updated: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RoomState {
    pub name: String,
    /// The current time of the room, in seconds since the room started
    pub roomtime: f64,
    /// List of users in room
    pub users: Vec<String>,
}

/// Struct containing possible message types sent to the client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UpdateMessage {
    /// Requesting a Heartbeat response
    Heartbeat,
    /// Sending information about the current room
    RoomInfo(RoomState),
    /// Sending information about objects in the room
    Update(f64, bool, DashMap<String, ObjectData>),
    /// Tell client to display text for a duration
    DisplayText(u16, String, u16),
    /// Tell client to play a beep from a given object, with a frequency and duration
    Beep(String, u16, u16),
}

/// Struct containing possible message types sent to the server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMessage {
    /// Responding to a Heartbeat request
    Heartbeat,
    /// Requesting reset
    ResetAll,
    /// Requesting robot reset
    ResetRobot(String),
    /// Claiming robot
    ClaimRobot(String),
    /// Joining Room (room id, username, password)
    JoinRoom(String, String, Option<String>)
}