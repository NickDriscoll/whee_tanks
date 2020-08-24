use gl::types::*;
use std::clone::Clone;
use std::collections::HashMap;
use std::ptr;
use ozy_engine::{glutil, routines};
use crate::DEFAULT_TEX_PARAMS;
use crate::input::{Command, InputType};

pub struct StaticGeometry {
    pub vao: GLuint,
    pub albedo: GLuint,
    pub normal: GLuint,
    pub model_matrix: glm::TMat4<f32>,
    pub index_count: GLsizei
}

//Something too simple for a skeleton
pub struct SimpleMesh {
    pub vao: GLuint,
    pub texture_maps: [GLuint; 3],
    pub index_count: GLint
}

impl SimpleMesh {
    pub fn from_ozy(path: &str, texture_keeper: &mut TextureKeeper) -> Self {
        match routines::load_ozymesh(path) {
            Some(meshdata) => unsafe {
                let vao = glutil::create_vertex_array_object(&meshdata.vertex_array.vertices, &meshdata.vertex_array.indices, &meshdata.vertex_array.attribute_offsets);
                let count = meshdata.geo_boundaries[1] as GLint;
                let albedo = texture_keeper.fetch_texture(&meshdata.texture_names[0], "albedo");
                let normal = texture_keeper.fetch_texture(&meshdata.texture_names[0], "normal");
                let roughness = texture_keeper.fetch_texture(&meshdata.texture_names[0], "roughness");
    
                SimpleMesh {
                    vao,
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

#[derive(Debug)]
pub struct Skeleton {
    pub vao: GLuint,
	pub node_list: Vec<usize>,
	pub geo_boundaries: Vec<u16>,			//[0, a, b, c, ..., indices.length - 1]
    pub albedo_maps: Vec<GLuint>,
    pub normal_maps: Vec<GLuint>,
    pub roughness_maps: Vec<GLuint>,
    pub bones: Vec<Bone>
}

impl Skeleton {
    pub fn get_bones(&self) -> Vec<Bone> {
        self.bones.clone()
    }
}

//Represents a single bone in a skeleton
//SkeletonNodes are stored in a flat array, and the value of parent is the index in the array of said node's parent
#[derive(Clone, Debug)]
pub struct Bone {
    pub transform: glm::TMat4<f32>,
    pub parent: Option<usize>
}

pub struct Tank<'a> {
    pub position: glm::TVec3<f32>,
    pub speed: f32,
    pub last_shot_time: f32,
    pub firing: bool,
    pub forward: glm::TVec3<f32>,
    pub move_state: TankMoving,
    pub rotating: f32,
    pub rotation: glm::TMat4<f32>,
    pub turret_forward: glm::TVec4<f32>,
    pub turret_origin: glm::TVec4<f32>,
    pub skeleton: &'a Skeleton,
    pub brain: Brain,
    pub bones: Vec<Bone>
}

impl<'a> Tank<'a> {
    const SPEED: f32 = 2.0;
    const HULL_INDEX: usize = 0;
    const TURRET_INDEX: usize = 1;
    pub const SHOT_COOLDOWN: f32 = 1.5;
    
    pub fn new(position: glm::TVec3<f32>, forward: glm::TVec3<f32>, skeleton: &'a Skeleton, brain: Brain) -> Self {        
        Tank {
            position,
            speed: Self::SPEED,
            last_shot_time: 0.0,
            firing: false,
            forward,
            move_state: TankMoving::Not,
            rotating: 0.0,
            rotation: glm::identity(),
            turret_forward: glm::vec4(1.0, 0.0, 0.0, 0.0),
            turret_origin: glm::vec4(0.0, 1.0, 0.0, 0.0),
            skeleton,
            brain,
            bones: skeleton.get_bones()
        }
    }

    pub fn aim_turret(&mut self, target: &glm::TVec4<f32>) {
        //Point turret at target
        self.turret_forward = glm::normalize(&(target - self.turret_origin));
        self.bones[Self::TURRET_INDEX].transform = {
            let new_x = -glm::cross(&glm::vec4_to_vec3(&-self.turret_forward), &glm::vec3(0.0, 1.0, 0.0));
            self.bones[Self::HULL_INDEX].transform *
            glm::mat4(new_x.x, 0.0, -self.turret_forward.x, 0.0,
                    new_x.y, 1.0, -self.turret_forward.y, 0.0,
                    new_x.z, 0.0, -self.turret_forward.z, 0.0,
                    0.0, 0.0, 0.0, 1.0
                    ) * glm::affine_inverse(self.rotation)
        };
    }
}

#[derive(Debug)]
pub struct Shell {
    pub position: glm::TVec4<f32>,
    pub velocity: glm::TVec4<f32>,
    pub transform: glm::TMat4<f32>,
    pub spawn_time: f32
}

impl Shell {
    pub const VELOCITY: f32 = 2.0;
    pub const LIFETIME: f32 = 5.0;
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
}

pub enum TankMoving {
    Forwards,
    Backwards,
    Not
}

//Determines what to do during the update step for a given entity
pub enum Brain {
    PlayerInput,
    DumbAI,
}

pub struct GameState {
    pub kind: GameStateKind,
    input_maps: HashMap<GameStateKind, HashMap<(InputType, glfw::Action), Command>>
}

impl GameState {
    pub fn new(kind: GameStateKind, input_maps: HashMap<GameStateKind, HashMap<(InputType, glfw::Action), Command>>) -> Self {
        GameState {
            kind,
            input_maps
        }
    }

    pub fn get_input_map(&self) -> HashMap<(InputType, glfw::Action), Command> {
        match self.input_maps.get(&self.kind) {
            Some(map) => { map.clone() }
            None => { HashMap::new() }
        }
    }
}

//State that controls what is updated and what is drawn
#[derive(Eq, Hash, PartialEq)]
pub enum GameStateKind {
    Playing,
    MainMenu,
    Paused,
    Pausing,
    Resuming
}

pub enum ImageEffect {
    Blur,
    None
}