use gl::types::*;
use std::clone::Clone;
use std::collections::HashMap;
use crate::input::{Command, InputKind};
use crate::render::{Framebuffer, RenderTarget, Skeleton};

pub struct Tank<'a> {
    pub position: glm::TVec3<f32>,
    pub speed: f32,
    pub last_shot_time: f32,
    pub live_shells: usize,
    pub firing: bool,
    pub forward: glm::TVec3<f32>,
    pub rotating: f32,
    pub rotation: glm::TMat4<f32>,
    pub turret_forward: glm::TVec4<f32>,
    pub skeleton: &'a Skeleton,
    pub brain: Brain,
    pub bone_transforms: Vec<glm::TMat4<f32>>
}

impl<'a> Tank<'a> {
    pub const SPEED: f32 = 4.0;
    pub const ROTATION_SPEED: f32 = 3.141592654;
    pub const SHOT_COOLDOWN: f32 = 0.05;
    pub const HULL_INDEX: usize = 0;
    pub const TURRET_INDEX: usize = 1;
    pub const HIT_SPHERE_RADIUS: f32 = 0.4;
    pub const MAX_LIVE_SHELLS: usize = 5;
    
    pub fn new(position: glm::TVec3<f32>, forward: glm::TVec3<f32>, skeleton: &'a Skeleton, brain: Brain) -> Self {        
        Tank {
            position,
            speed: 0.0,
            last_shot_time: 0.0,
            live_shells: 0,
            firing: false,
            forward,
            rotating: 0.0,
            rotation: glm::identity(),
            turret_forward: glm::vec4(1.0, 0.0, 0.0, 0.0),
            skeleton,
            brain,
            bone_transforms: vec![glm::identity(); skeleton.bones.len()]
        }
    }
}

#[derive(Debug)]
pub struct Shell {
    pub position: glm::TVec4<f32>,
    pub velocity: glm::TVec4<f32>,
    pub transform: glm::TMat4<f32>,
    pub spawn_time: f32,
    pub shooter: usize
}

impl Shell {
    pub const VELOCITY: f32 = 6.0;
    pub const LIFETIME: f32 = 4.0;
    pub const HIT_SPHERE_RADIUS: f32 = 0.05;
}

//Determines what to do during the update step for a given entity
pub enum Brain {
    PlayerInput,
    DumbAI,
}

pub struct GameState {
    pub kind: GameStateKind,
    input_maps: HashMap<GameStateKind, HashMap<(InputKind, glfw::Action), Command>>
}

impl GameState {
    pub fn new(kind: GameStateKind, input_maps: HashMap<GameStateKind, HashMap<(InputKind, glfw::Action), Command>>) -> Self {
        GameState {
            kind,
            input_maps
        }
    }

    pub fn get_input_map(&self) -> HashMap<(InputKind, glfw::Action), Command> {
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
    Paused
}

pub enum ImageEffect {
    Blur,
    None
}

pub struct ScreenState {
    pub window_size: (u32, u32),
    pub aspect_ratio: f32,
    pub ping_pong_fbos: [RenderTarget; 2],
    pub default_framebuffer: Framebuffer,
    pub clipping_from_view: glm::TMat4<f32>,
    pub clipping_from_world: glm::TMat4<f32>,
    pub world_from_clipping: glm::TMat4<f32>,
    pub clipping_from_screen: glm::TMat4<f32>
}

impl ScreenState {
    const ORTHO_SIZE: f32 = 5.0;

    pub fn new(window_size: (u32, u32), view_from_world: &glm::TMat4<f32>) -> Self {
        let aspect_ratio = window_size.0 as f32 / window_size.1 as f32;
        let clipping_from_view = glm::ortho(-Self::ORTHO_SIZE*aspect_ratio, Self::ORTHO_SIZE*aspect_ratio, -Self::ORTHO_SIZE, Self::ORTHO_SIZE, -Self::ORTHO_SIZE, Self::ORTHO_SIZE * 2.0);
        let clipping_from_world = clipping_from_view * view_from_world;
        let world_from_clipping = glm::affine_inverse(clipping_from_world);
        let clipping_from_screen = glm::mat4(
            2.0 / window_size.0 as f32, 0.0, 0.0, -1.0,
            0.0, -(2.0 / window_size.1 as f32), 0.0, 1.0,
            0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 1.0
        );

        //Initialize the two offscreen rendertargets used for post-processing
        let ping_pong_fbos = unsafe {
            let size = (window_size.0 as GLint, window_size.1 as GLint);
            [RenderTarget::new(size), RenderTarget::new(size)]            
        };

        //Initialize default framebuffer
        let default_framebuffer = Framebuffer {
            name: 0,
            size: (window_size.0 as GLsizei, window_size.1 as GLsizei),
            clear_flags: gl::DEPTH_BUFFER_BIT | gl::COLOR_BUFFER_BIT,
            cull_face: gl::BACK
        };    

        ScreenState {
            window_size,
            aspect_ratio,
            ping_pong_fbos,
            default_framebuffer,
            clipping_from_view,
            clipping_from_world,
            world_from_clipping,
            clipping_from_screen
        }
    }
}

#[derive(Debug)]
pub enum CollisionEntity {
    Tank(usize),
    Shell(usize)
}

#[derive(Debug)]
pub struct CollisionSphere {
    pub origin: glm::TVec4<f32>,
    pub radius: f32,
    pub target: CollisionEntity
}

impl CollisionSphere {
    pub fn new(transform: &glm::TMat4<f32>, radius: f32, target: CollisionEntity) -> Self {
        let origin = transform * glm::vec4(0.0, 0.0, 0.0, 1.0);
        CollisionSphere {
            origin,
            radius,
            target
        }
    }
}