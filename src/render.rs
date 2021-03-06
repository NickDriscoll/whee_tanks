use gl::types::*;
use ozy_engine::{glutil, routines};
use std::collections::HashMap;
use std::mem;
use std::ptr;
use std::os::raw::c_void;
use crate::DEFAULT_TEX_PARAMS;

pub const MAP_COUNT: usize = 3;      //[albedo, normal, roughness]

pub struct StaticGeometry {
    pub vao: GLuint,
    pub albedo: GLuint,
    pub normal: GLuint,
    pub model_matrix: glm::TMat4<f32>,
    pub index_count: GLsizei
}

//One contiguous piece of geometry
pub struct SimpleMesh {
    pub vao: GLuint,
    pub index_count: GLint,
    pub origin: glm::TVec4<f32>,
    pub texture_maps: [GLuint; MAP_COUNT]
}

impl SimpleMesh {
    pub fn from_ozy(path: &str, texture_keeper: &mut TextureKeeper) -> Self {
        match routines::load_ozymesh(path) {
            Some(meshdata) => unsafe {
                let vao = glutil::create_vertex_array_object(&meshdata.vertex_array.vertices, &meshdata.vertex_array.indices, &meshdata.vertex_array.attribute_offsets);
                let count = meshdata.geo_boundaries[1] as GLint;
                let origin = meshdata.origins[0];
                let albedo = texture_keeper.fetch_texture(&meshdata.texture_names[0], "albedo");
                let normal = texture_keeper.fetch_texture(&meshdata.texture_names[0], "normal");
                let roughness = texture_keeper.fetch_texture(&meshdata.texture_names[0], "roughness");
    
                SimpleMesh {
                    vao,
                    origin,
                    texture_maps: [albedo, normal, roughness],
                    index_count: count as GLint
                }
            }
            None => {
                panic!("Unable to load model.");
            }
        }
    }
}

pub struct InstancedMesh {
    vao: GLuint,
    transform_buffer: GLuint,
    index_count: GLint,
    active_instances: usize,
    max_instances: usize
}

impl InstancedMesh {
    const FLOATS_PER_TRANSFORM: usize = 16;

    pub fn new(vao: GLuint, index_count: GLint, max_instances: usize, instanced_attribute: GLuint) -> Self {
        //Create GPU buffer for instanced matrices
        let transform_buffer = unsafe {
            gl::BindVertexArray(vao);

            let mut b = 0;
            gl::GenBuffers(1, &mut b);
            gl::BindBuffer(gl::ARRAY_BUFFER, b);
            gl::BufferData(gl::ARRAY_BUFFER, (max_instances * Self::FLOATS_PER_TRANSFORM * mem::size_of::<GLfloat>()) as GLsizeiptr, ptr::null(), gl::DYNAMIC_DRAW);

            //Attach this buffer to the shell_mesh vao
            //We have to individually bind each column of the matrix as a different vec4 vertex attribute
            for i in 0..4 {
                let attribute_index = instanced_attribute + i;
                gl::VertexAttribPointer(attribute_index,
                                        4,
                                        gl::FLOAT,
                                        gl::FALSE,
                                        (Self::FLOATS_PER_TRANSFORM * mem::size_of::<GLfloat>()) as GLsizei,
                                        (i * 4 * mem::size_of::<GLfloat>() as GLuint) as *const c_void);
                gl::EnableVertexAttribArray(attribute_index);
                gl::VertexAttribDivisor(attribute_index, 1);
            }

            b
        };
        
        InstancedMesh {
            vao,
            max_instances,
            index_count,
            active_instances: 0,
            transform_buffer
        }
    }

    pub unsafe fn draw(&self) {
        gl::BindVertexArray(self.vao);
		gl::DrawElementsInstanced(gl::TRIANGLES, self.index_count, gl::UNSIGNED_SHORT, ptr::null(), self.active_instances as GLint);
    }

    pub fn max_instances(&self) -> usize { self.max_instances }

    pub fn update_buffer(&mut self, transforms: &[f32]) {
        //Record the current active instance count
        self.active_instances = transforms.len() / Self::FLOATS_PER_TRANSFORM;

        //Update GPU buffer storing hit volume transforms
		if transforms.len() > 0 {
			unsafe {
				gl::BindBuffer(gl::ARRAY_BUFFER, self.transform_buffer);
				gl::BufferSubData(gl::ARRAY_BUFFER,
								0 as GLsizeiptr,
								(transforms.len() * mem::size_of::<GLfloat>()) as GLsizeiptr,
								&transforms[0] as *const GLfloat as *const c_void
								);
			}
		}
    }
}

#[derive(Debug)]
pub struct Skeleton {
    pub vao: GLuint,
	pub node_list: Vec<usize>,
	pub geo_boundaries: Vec<u16>,			//[0, a, b, c, ..., indices.length - 1]
    pub albedo_maps: Vec<GLuint>,
    pub normal_maps: Vec<GLuint>,
    pub roughness_maps: Vec<GLuint>,
    pub bones: Vec<Bone>,
    pub bone_origins: Vec<glm::TVec4<f32>>
}

impl Skeleton {
    pub fn get_bones(&self) -> Vec<Bone> {
        self.bones.clone()
    }

    pub unsafe fn draw_bone(&self, index: usize) {
        gl::DrawElements(gl::TRIANGLES, (self.geo_boundaries[index + 1] - self.geo_boundaries[index]) as i32, gl::UNSIGNED_SHORT, (mem::size_of::<GLushort>() * self.geo_boundaries[index] as usize) as *const c_void);
    }
}

//Represents a single bone in a skeleton
//SkeletonNodes are stored in a flat array, and the value of parent is the index in the array of said node's parent
#[derive(Clone, Debug)]
pub struct Bone {
    pub transform: glm::TMat4<f32>,
    pub parent: Option<usize>
}

pub struct TextureKeeper {
    map: HashMap<String, u32>
}

impl TextureKeeper {
    pub fn new() -> Self {
        TextureKeeper {
            map: HashMap::new()
        }
    }

    pub unsafe fn fetch_texture(&mut self, name: &str, map_type: &str) -> GLuint {        
		let texture_path = format!("textures/{}/{}.png", name, map_type);
		match self.map.get(&texture_path) {
			Some(t) => { *t }
			None => {
				let name = glutil::load_texture(&texture_path, &DEFAULT_TEX_PARAMS);
				self.map.insert(texture_path, name);
				name
			}
        }
    }
}

pub struct Framebuffer {
    pub name: GLuint,
    pub size: (GLsizei, GLsizei),
    pub clear_flags: GLenum,
    pub cull_face: GLenum
}

impl Framebuffer {
    pub unsafe fn bind(&self) {
        gl::BindFramebuffer(gl::FRAMEBUFFER, self.name);
        gl::Viewport(0, 0, self.size.0, self.size.1);
        gl::Clear(self.clear_flags);
        gl::CullFace(self.cull_face);
    }
}

impl Drop for Framebuffer {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteFramebuffers(1, &self.name);
        }
    }
}

//A framebuffer object with color and depth attachments
pub struct RenderTarget {
    pub framebuffer: Framebuffer,
    pub texture: GLuint
}

impl RenderTarget {
    pub unsafe fn new(size: (GLint, GLint)) -> Self {
        let mut fbo = 0;
		let mut texs = [0; 2];
		gl::GenFramebuffers(1, &mut fbo);
		gl::GenTextures(2, &mut texs[0]);
		let (color_tex, depth_tex) = (texs[0], texs[1]);

		//Initialize the color buffer
		gl::BindTexture(gl::TEXTURE_2D, color_tex);
		gl::TexImage2D(
			gl::TEXTURE_2D,
			0,
			gl::SRGB8_ALPHA8 as GLint,
			size.0,
			size.1,
			0,
			gl::RGBA,
			gl::FLOAT,
			ptr::null()
		);
		let params = [
			(gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE),
			(gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE),
			(gl::TEXTURE_MIN_FILTER, gl::LINEAR_MIPMAP_LINEAR),
			(gl::TEXTURE_MAG_FILTER, gl::NEAREST)
		];
        glutil::apply_texture_parameters(&params);
	    gl::GenerateMipmap(gl::TEXTURE_2D);

		gl::BindTexture(gl::TEXTURE_2D, depth_tex);
		gl::TexImage2D(
			gl::TEXTURE_2D,
			0,
			gl::DEPTH_COMPONENT as GLint,
			size.0,
			size.1,
			0,
			gl::DEPTH_COMPONENT,
			gl::FLOAT,
			ptr::null()
		);
		let params = [
			(gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE),
			(gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE),
			(gl::TEXTURE_MIN_FILTER, gl::NEAREST),
			(gl::TEXTURE_MAG_FILTER, gl::NEAREST)
		];
		glutil::apply_texture_parameters(&params);

		gl::BindFramebuffer(gl::FRAMEBUFFER, fbo);
		gl::FramebufferTexture2D(
			gl::FRAMEBUFFER,
			gl::COLOR_ATTACHMENT0,
			gl::TEXTURE_2D,
			color_tex,
			0
		);
		gl::FramebufferTexture2D(
			gl::FRAMEBUFFER,
			gl::DEPTH_ATTACHMENT,
			gl::TEXTURE_2D,
			depth_tex,
			0
		);
		gl::BindFramebuffer(gl::FRAMEBUFFER, 0);

		let f_buffer = Framebuffer {
			name: fbo,
			size: (size.0 as GLsizei, size.1 as GLsizei),
			clear_flags: gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT,
			cull_face: gl::BACK
		};

		RenderTarget {
			framebuffer: f_buffer,
			texture: color_tex
		}
    }

    pub unsafe fn bind(&self) {
        self.framebuffer.bind();
    }

    pub unsafe fn resize(&mut self, size: (u32, u32)) {
        gl::DeleteFramebuffers(1, &self.framebuffer.name);
        let n = Self::new((size.0 as GLint, size.1 as GLint));
        self.framebuffer = n.framebuffer;
        self.texture = n.texture;
    }
}