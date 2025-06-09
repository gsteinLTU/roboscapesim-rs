use std::{collections::BTreeMap, f32::consts::PI};

use log::info;
use rapier3d::math::AngVector;
use roboscapesim_common::{Shape, VisualInfo};
use serde_json::Value;

use crate::util::util::{num_val, str_val};


pub fn parse_rotation(rotation: &Value) -> nalgebra::Matrix<f32, nalgebra::Const<3>, nalgebra::Const<1>, nalgebra::ArrayStorage<f32, 3, 1>> {
    let rotation = match rotation {
        serde_json::Value::Number(n) => AngVector::new(0.0, n.as_f64().unwrap() as f32  * PI / 180.0, 0.0),
        serde_json::Value::String(s) => AngVector::new(0.0, s.parse::<f32>().unwrap_or_default()  * PI / 180.0, 0.0),
        serde_json::Value::Array(a) => {
            if a.len() >= 3 {
                AngVector::new(num_val(&a[0]) * PI / 180.0, num_val(&a[1]) * PI / 180.0, num_val(&a[2]) * PI / 180.0)
            } else if !a.is_empty() {
                AngVector::new(0.0, num_val(&a[0]) * PI / 180.0, 0.0)
            } else {
                AngVector::new(0.0, 0.0, 0.0)
            }
        },
        _ => AngVector::new(0.0, 0.0, 0.0)
    };
    rotation
}

pub fn parse_visual_info(options: &BTreeMap<String, Value>, shape: Shape) -> Option<VisualInfo> {
    if options.len() == 0 {
        return None;
    }
    
    if options.contains_key("texture") {
        let mut uscale = 1.0;
        let mut vscale = 1.0;

        if options.contains_key("uscale") {
            uscale = num_val(options.get("uscale").unwrap());
        }

        if options.contains_key("vscale") {
            vscale = num_val(options.get("vscale").unwrap());
        }

        return Some(VisualInfo::Texture(str_val(options.get("texture").unwrap()), uscale, vscale, shape));
    } else if options.contains_key("color") {
        // Parse color data
        return Some(parse_visual_info_color(options.get("color").unwrap(), shape));
    } else if options.contains_key("mesh") {
        // Use mesh
        let mut mesh_name = str_val(options.get("mesh").unwrap());

        // Assume non-specified extension is glb
        if !mesh_name.contains('.') {
            mesh_name += ".glb";
        }

        return Some(VisualInfo::Mesh(mesh_name));
    }

    None
}

pub fn parse_visual_info_color(visualinfo: &serde_json::Value, shape: roboscapesim_common::Shape) -> VisualInfo {
    let mut parsed_visualinfo = VisualInfo::default_with_shape(shape);

    if !visualinfo.is_null() {
        match visualinfo {
            serde_json::Value::String(s) => { 
                if !s.is_empty() {
                    if s.starts_with('#') || s.starts_with("rgb") {
                        // attempt to parse as hex/CSS color
                        let r: Result<colorsys::Rgb, _> = s.parse();

                        if let Ok(color) = r {
                            parsed_visualinfo = VisualInfo::Color(color.red() as f32, color.green() as f32, color.blue() as f32, shape);
                        } else if r.is_err() {
                            let r = colorsys::Rgb::from_hex_str(s);
                            if let Ok(color) = r {
                                parsed_visualinfo = VisualInfo::Color(color.red() as f32 / 255.0, color.green() as f32 / 255.0, color.blue() as f32 / 255.0, shape);
                            } else if r.is_err() {
                                info!("Failed to parse {s} as color");
                            }
                        }
                    } else {
                        // attempt to parse as color name
                        let color = color_name::Color::val().by_string(s.to_owned());

                        if let Ok(color) = color {
                            parsed_visualinfo = VisualInfo::Color(color[0] as f32 / 255.0, color[1] as f32 / 255.0, color[2] as f32 / 255.0, shape);
                        }
                    }
                }
            },
            serde_json::Value::Array(a) =>  { 
                if a.len() == 3 {
                    // Color as array
                    parsed_visualinfo = VisualInfo::Color(num_val(&a[0]) / 255.0, num_val(&a[1]) / 255.0, num_val(&a[2]) / 255.0, shape);
                } else if a.len() == 4 {
                    // Color as array with alpha
                    parsed_visualinfo = VisualInfo::Color(num_val(&a[0]) / 255.0, num_val(&a[1]) / 255.0, num_val(&a[2]) / 255.0, shape);
                } else if a.len() == 1 {
                    parsed_visualinfo = parse_visual_info_color(&a[0], shape);
                }
            },
            _ => {
                info!("Received invalid visualinfo");
            }
        }
    }
    
    parsed_visualinfo
}