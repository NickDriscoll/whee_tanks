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
	pub nodes: Vec<usize>,
	pub geo_boundaries: Vec<u16>,			//[0, a, b, c, ..., indices.length - 1]
	pub albedo_maps: Vec<GLuint>
}

//Represents a single bone in a skeleton
pub struct SkeletonNode {
    pub transform: glm::TMat4<f32>,
    pub parent: Option<usize>
}