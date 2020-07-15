use gl::types::*;
use std::collections::HashMap;
use ozy_engine::glutil;
use crate::DEFAULT_TEX_PARAMS;

pub struct StaticGeometry {
    pub vao: GLuint,
    pub albedo: GLuint,
    pub normal: GLuint,
    pub model_matrix: glm::TMat4<f32>,
    pub index_count: GLsizei
}

//Something too simple for a skeleton
pub struct IndividualMesh {
    pub vao: GLuint,
    pub albedo_map: GLuint,
    pub normal_map: GLuint,
    pub index_count: GLint
}

#[derive(Debug)]
pub struct Skeleton {
    pub vao: GLuint,
    pub node_data: Vec<SkeletonNode>,
	pub node_list: Vec<usize>,
	pub geo_boundaries: Vec<u16>,			//[0, a, b, c, ..., indices.length - 1]
    pub albedo_maps: Vec<GLuint>,
    pub normal_maps: Vec<GLuint>
}

//Represents a single bone in a skeleton
//SkeletonNodes are stored in a flat array, and the value of parent is the index in the array of said node's parent
#[derive(Debug)]
pub struct SkeletonNode {
    pub transform: glm::TMat4<f32>,
    pub parent: Option<usize>
}

pub struct Tank {
    pub position: glm::TVec3<f32>,
    pub speed: f32,
    pub firing: bool,
    pub forward: glm::TVec3<f32>,
    pub move_state: TankMoving,
    pub tank_rotating: Rotating,
    pub turret_forward: glm::TVec4<f32>,
    pub skeleton: Skeleton
}

#[derive(Debug)]
pub struct Shell {
    pub position: glm::TVec4<f32>,
    pub velocity: glm::TVec4<f32>,
    pub transform: glm::TMat4<f32>,
    pub spawn_time: f32,
    pub vao: GLuint
}

pub struct TextureKeeper {
    pub map: HashMap<String, u32>
}

impl TextureKeeper {
    pub fn new() -> Self {
        TextureKeeper {
            map: HashMap::new()
        }
    }

    pub unsafe fn fetch_texture(&mut self, name: &str, map_type: &str) -> GLuint {        
		let texture_path = format!("textures/{}/{}.png", name, map_type);
		let id = match self.map.get(&texture_path) {
			Some(t) => {
				*t
			}
			None => {
				let name = glutil::load_texture(&texture_path, &DEFAULT_TEX_PARAMS);
				self.map.insert(texture_path, name);
				name
			}
        };
        id
    }
}

pub enum TankMoving {
    Forwards,
    Backwards,
    Not
}

pub enum Rotating {
    Left,
    Right,
    Not
}