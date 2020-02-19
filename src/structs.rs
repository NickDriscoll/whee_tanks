use gl::types::*;

pub struct StaticGeometry {
    pub vao: GLuint,
    pub texture: GLuint,
    pub model_matrix: glm::TMat4<f32>,
    pub index_count: GLsizei
}