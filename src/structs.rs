use gl::types::*;

pub struct StaticGeometry {
    pub vao: GLuint,
    pub albedo: GLuint,
    pub normal: GLuint,
    pub model_matrix: glm::TMat4<f32>,
    pub index_count: GLsizei
}

#[derive(Debug)]
pub struct Skeleton {
    pub vao: GLuint,
    pub node_data: Vec<SkeletonNode>,
	pub node_list: Vec<usize>,
	pub geo_boundaries: Vec<u16>,			//[0, a, b, c, ..., indices.length - 1]
	pub albedo_maps: Vec<GLuint>
}

//Represents a single bone in a skeleton
#[derive(Debug)]
pub struct SkeletonNode {
    pub transform: glm::TMat4<f32>,
    pub parent: Option<usize>
}

pub struct Tank {
    pub position: glm::TVec3<f32>,
    pub speed: f32,
    pub forward: glm::TVec3<f32>,
    pub move_state: TankMoving,
    pub tank_rotating: Rotating,
    pub turret_forward: glm::TVec3<f32>,
    pub skeleton: Skeleton
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