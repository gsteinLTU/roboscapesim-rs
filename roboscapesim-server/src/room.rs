use dashmap::DashMap;
use nalgebra::vector;
use roboscapesim_common::*;

pub struct RoomData {
    pub objects: DashMap<String, ObjectData>
}

impl RoomData {
    pub fn new() -> RoomData {
        let mut obj = RoomData {
            objects: DashMap::new()
        };

        // Setup test room
        obj.objects.insert("robot".into(), ObjectData { 
            name: "robot".into(),
            transform: Transform { ..Default::default() }, 
            visual_info: VisualInfo::Mesh("parallax_robot.glb".into()) 
        });
        obj.objects.insert("ground".into(), ObjectData { 
            name: "ground".into(),
            transform: Transform { scaling: vector![100.0, 0.05, 100.0], position: vector![0.0, -0.095, 0.0], ..Default::default() }, 
            visual_info: VisualInfo::Color(0.8, 0.6, 0.45) 
        });

        obj
    }
}