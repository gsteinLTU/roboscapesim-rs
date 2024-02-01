use std::{collections::HashMap, fmt::Display};
use nalgebra::{Quaternion, Vector3, vector, Point3, UnitQuaternion};
use serde::{Deserialize, Serialize};

pub mod api;

/// Trait for interpolating between two values
pub trait Interpolatable<T> 
where Self: Sized {
    /// Interpolate between two values, assuming the interpolation is valid
    fn interpolate(&self, other: &T, t: f32) -> Self {
        self.try_interpolate(other, t).unwrap()
    }
    
    /// Interpolate between two values, returning an Err if the interpolation is invalid
    fn try_interpolate(&self, other: &T, t: f32) -> Result<Self, &'static str>;
}

/// 3D Transform data for an object
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Transform {
    #[serde(rename="p")]
    pub position: Point3<f32>,
    #[serde(rename="r")]
    pub rotation: Orientation,
    #[serde(rename="s")]
    pub scaling: Vector3<f32>,
}

impl Default for Transform {
    /// Create a default Transform at (0,0,0) with no rotation or scaling
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
    #[serde(rename="e")]
    Euler(Vector3<f32>),
    #[serde(rename="q")]
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
                    Ok(Orientation::Quaternion(q.lerp(&q2, t).normalize()))
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

impl Into<(f32, f32, f32)> for Orientation {
    fn into(self) -> (f32, f32, f32) {
        match self {
            Orientation::Euler(e) => (e.x, e.y, e.z),
            Orientation::Quaternion(q) =>  {
                UnitQuaternion::from_quaternion(q).euler_angles()
            },
        }
    }
}

impl From<Vector3<f32>> for Orientation {
    fn from(value: Vector3<f32>) -> Self {
        Self::Euler(value)
    }
}

impl From<UnitQuaternion<f32>> for Orientation {
    fn from(value: UnitQuaternion<f32>) -> Self {
        Self::Quaternion(value.quaternion().to_owned())
    }
}

impl From<Quaternion<f32>> for Orientation {
    fn from(value: Quaternion<f32>) -> Self {
        Self::Quaternion(value)
    }
}

impl Into<Vector3<f32>> for Orientation {
    fn into(self) -> Vector3<f32> {
        match self {
            Orientation::Euler(e) => e,
            Orientation::Quaternion(q) =>  {
                let e = UnitQuaternion::from_quaternion(q).euler_angles();
                vector![e.0, e.1, e.2]
            },
        }
    }
}

/// Possible shapes for an object
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum Shape {
    Box, Sphere, Cylinder, Capsule
}

impl Display for Shape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            Shape::Box => "box",
            Shape::Sphere => "sphere",
            Shape::Cylinder => "cylinder",
            Shape::Capsule => "capsule",
        })
    }
}

/// Information about how to display an object, if at all
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum VisualInfo {
    None,
    Color(f32, f32, f32, Shape),
    Texture(String, f32, f32, Shape),
    Mesh(String),
}

impl Default for VisualInfo {
    fn default() -> Self {
        Self::Color(1.0, 1.0, 1.0, Shape::Box)
    }
}

impl VisualInfo {
    /// Create a default VisualInfo with a given shape
    pub fn default_with_shape(shape: Shape) -> Self {
        Self::Color(1.0, 1.0, 1.0, shape)
    }
}

/// Generic data about an object to be sent to the client
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ObjectData {
    #[serde(rename="n")]
    pub name: String,
    #[serde(rename="t")]
    pub transform: Transform,
    #[serde(rename="v")]
    pub visual_info: Option<VisualInfo>,
    /// If true, the object should be assumed to not move through physics
    #[serde(rename="k")]
    pub is_kinematic: bool,
    /// If true, the object has been modified since last send
    #[serde(rename="u")]
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
    #[serde(rename="h")]
    Heartbeat,
    /// Sending information about the current room
    #[serde(rename="ri")]
    RoomInfo(RoomState),
    /// Sending information about objects in the room
    #[serde(rename="u")]
    Update(f64, bool, HashMap<String, ObjectData>),
    /// Tell client to display text for a duration (id, text, timeout)
    #[serde(rename="dt")]
    DisplayText(String, String, Option<f64>),
    /// Clear all text displayed
    #[serde(rename="ct")]
    ClearText,
    /// Tell client to play a beep from a given object, with a frequency and duration
    #[serde(rename="bp")]
    Beep(String, u16, u16),
    /// Hibernation started
    #[serde(rename="hib")]
    Hibernating,
    /// Client should remove an object with a given id
    #[serde(rename="ro")]
    RemoveObject(String),
    /// Client should remove all objects in room
    #[serde(rename="ra")]
    RemoveAll(),
    /// Robot claimed and by whom
    #[serde(rename="rc")]
    RobotClaimed(String, String),
    /// Error in VM
    #[serde(rename="e")]
    VMError(String, usize),
}

/// Struct containing possible message types sent to the server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMessage {
    /// Responding to a Heartbeat request
    #[serde(rename="h")]
    Heartbeat,
    /// Requesting reset
    #[serde(rename="R")]
    ResetAll,
    /// Requesting robot reset
    #[serde(rename="rr")]
    ResetRobot(String),
    /// Claiming robot
    #[serde(rename="c")]
    ClaimRobot(String),
    /// Claiming robot
    #[serde(rename="uc")]
    UnclaimRobot(String),
    /// Request encryption for robot
    #[serde(rename="er")]
    EncryptRobot(String),
    /// Joining Room (room id, username, password)
    #[serde(rename="j")]
    JoinRoom(String, String, Option<String>)
}