extern crate nalgebra_glm as glm;
use std::{mem, ptr};
use std::collections::HashMap;
use std::os::raw::c_void;
use std::time::Instant;
use glfw::{Action, Context, Key, MouseButton, WindowEvent, WindowMode};
use gl::types::*;
use glyph_brush::{ab_glyph::{FontArc, PxScale}, BrushAction, BrushError, GlyphBrushBuilder, GlyphCruncher, GlyphVertex, Section, Text};
use ozy_engine::{glutil, routines};
use ozy_engine::structs::OptionVec;
use crate::structs::*;
use crate::input::{Command, InputType};
use crate::ui::{ButtonState, Menu, MenuAnchor, UIButton};

mod structs;
mod input;
mod ui;

const DEFAULT_TEX_PARAMS: [(GLenum, GLenum); 4] = [
	(gl::TEXTURE_WRAP_S, gl::REPEAT),
	(gl::TEXTURE_WRAP_T, gl::REPEAT),
	(gl::TEXTURE_MIN_FILTER, gl::LINEAR),
	(gl::TEXTURE_MAG_FILTER, gl::LINEAR)
];

//Binds each texture map specified in maps to each texture mapping unit in order
unsafe fn bind_texture_maps(maps: &[GLuint]) {
	for i in 0..maps.len() {
		gl::ActiveTexture(gl::TEXTURE0 + i as GLenum);
		gl::BindTexture(gl::TEXTURE_2D, maps[i]);
	}
}

unsafe fn initialize_texture_samplers(program: GLuint, identifiers: &[&str]) {
	gl::UseProgram(program);
	for i in 0..identifiers.len() {
		glutil::bind_int(program, identifiers[i], i as GLint);
	}
}

//Second argument to glyph_brush.process_queued()
fn glyph_vertex_transform(vertex: GlyphVertex) -> [f32; 16] {	
	let left = vertex.pixel_coords.min.x as f32;
	let right = vertex.pixel_coords.max.x as f32;
	let top = vertex.pixel_coords.min.y as f32;
	let bottom = vertex.pixel_coords.max.y as f32;
	let texleft = vertex.tex_coords.min.x;
	let texright = vertex.tex_coords.max.x;
	let textop = vertex.tex_coords.min.y;
	let texbottom = vertex.tex_coords.max.y;

	//We need to return four vertices in screen space
	[
		left, bottom, texleft, texbottom,
		right, bottom, texright, texbottom,
		left, top, texleft, textop,
		right, top, texright, textop
	]	
}

extern "system" fn gl_debug_callback(source: GLenum, gltype: GLenum, id: GLuint, severity: GLenum, length: GLsizei, message: *const GLchar, user_param: *mut c_void) {
	println!("--------------------OpenGL debug message--------------------");
	println!("ID: {}", id);
	
	match source {
		gl::DEBUG_SOURCE_API => 				{ println!("Source: API"); }
		gl::DEBUG_SOURCE_WINDOW_SYSTEM => 		{ println!("Source: Window System"); }
		gl::DEBUG_SOURCE_SHADER_COMPILER => 	{ println!("Source: Shader Compiler"); }
		gl::DEBUG_SOURCE_THIRD_PARTY => 		{ println!("Source: Third Party"); }
		gl::DEBUG_SOURCE_APPLICATION => 		{ println!("Source: Application"); }
		gl::DEBUG_SOURCE_OTHER => 				{ println!("Source: Other"); }
		_ => {}
	}

	match gltype {
		gl::DEBUG_TYPE_ERROR => 					{ println!("Type: Error") }
		gl::DEBUG_TYPE_DEPRECATED_BEHAVIOR => 		{ println!("Type: Deprecated Behaviour") }
		gl::DEBUG_TYPE_UNDEFINED_BEHAVIOR => 		{ println!("Type: Undefined Behaviour") }
		gl::DEBUG_TYPE_PORTABILITY => 				{ println!("Type: Portability") }
		gl::DEBUG_TYPE_PERFORMANCE => 				{ println!("Type: Performance") }
		gl::DEBUG_TYPE_MARKER => 					{ println!("Type: Marker") }
		gl::DEBUG_TYPE_PUSH_GROUP => 				{ println!("Type: Push Group") }
		gl::DEBUG_TYPE_POP_GROUP => 				{ println!("Type: Pop Group") }
		gl::DEBUG_TYPE_OTHER => 					{ println!("Type: Other") }
		_ => {}
	}

	match severity {
		gl::DEBUG_SEVERITY_HIGH => { 
			println!("Severity: High"); 
		}
		gl::DEBUG_SEVERITY_MEDIUM => { 
			println!("Severity: Medium"); 
	}
		gl::DEBUG_SEVERITY_LOW => { 
			println!("Severity: Low"); 
		}
		gl::DEBUG_SEVERITY_NOTIFICATION => { 
			println!("Severity: Notification"); 
		}
		_ => {}
	}

	let m = unsafe {
		let mut buffer = vec![0; length as usize];
		for i in 0..length as isize {
			buffer[i as usize] = *message.offset(i) as u8;
		}
		String::from_utf8(buffer).unwrap()
	};

	println!("Message: {}", m);
}

fn insert_index_buffer_quad(index_buffer: &mut [u16], i: usize) {
	index_buffer[i * 6] = 4 * i as u16;
	index_buffer[i * 6 + 1] = index_buffer[i * 6] + 1;
	index_buffer[i * 6 + 2] = index_buffer[i * 6] + 2;
	index_buffer[i * 6 + 3] = index_buffer[i * 6] + 3;
	index_buffer[i * 6 + 4] = index_buffer[i * 6] + 2;
	index_buffer[i * 6 + 5] = index_buffer[i * 6] + 1;
}

const FLOATS_PER_COLOR: usize = 4;
const COLORS_PER_BUTTON: usize = 4;
unsafe fn update_ui_button_color_buffer(buffer: GLuint, index: usize, color: [f32; FLOATS_PER_COLOR]) {
	let mut data = vec![0.0; FLOATS_PER_COLOR * COLORS_PER_BUTTON];
	for i in 0..(data.len() / FLOATS_PER_COLOR) {
		data[i * FLOATS_PER_COLOR] = color[0];
		data[i * FLOATS_PER_COLOR + 1] = color[1];
		data[i * FLOATS_PER_COLOR + 2] = color[2];
		data[i * FLOATS_PER_COLOR + 3] = color[3];
	}
	gl::BindBuffer(gl::ARRAY_BUFFER, buffer);
	gl::BufferSubData(gl::ARRAY_BUFFER,
					(COLORS_PER_BUTTON * FLOATS_PER_COLOR * index * mem::size_of::<GLfloat>()) as GLintptr,
					(FLOATS_PER_COLOR * COLORS_PER_BUTTON * mem::size_of::<GLfloat>()) as GLsizeiptr,
					&data[0] as *const f32 as *const c_void);
}

pub unsafe fn draw_ui_element(vao: GLuint, shader: GLuint, count: usize, clipping_from_screen: &glm::TMat4<f32>) {
    gl::UseProgram(shader);
	glutil::bind_matrix4(shader, "clipping_from_screen", &clipping_from_screen);
	gl::BindVertexArray(vao);
	gl::DrawElements(gl::TRIANGLES, 6 * count as GLint, gl::UNSIGNED_SHORT, ptr::null());    
}

fn main() {
	let mut window_size = (1920, 1080);
	let mut aspect_ratio = window_size.0 as f32 / window_size.1 as f32;
	//Init glfw
	let mut glfw = match glfw::init(glfw::FAIL_ON_ERRORS) {
		Ok(g) => { g }
		Err(e) => {	panic!("GLFW init error: {}", e); }
	};

	glfw.window_hint(glfw::WindowHint::ContextVersion(4, 3));
	glfw.window_hint(glfw::WindowHint::OpenGlProfile(glfw::OpenGlProfileHint::Core));
	glfw.window_hint(glfw::WindowHint::OpenGlDebugContext(true));

	//Create window
    let (mut window, events) = glfw.create_window(window_size.0, window_size.1, "Whee! Tanks! for ipad", WindowMode::Windowed).unwrap();

	//Make the window non-resizable
	window.set_resizable(false);

	//Make the window fullscreen
	/*
	glfw.with_primary_monitor_mut(|_, opt_monitor| {
			if let Some(monitor) = opt_monitor {
				let window_mode = WindowMode::FullScreen(monitor);
				let pos = monitor.get_pos();
				if let Some(mode) = monitor.get_video_mode() {
					window_size = (mode.width, mode.height);
					aspect_ratio = window_size.0 as f32 / window_size.1 as f32;
					window.set_size(window_size.0 as i32, window_size.1 as i32);
					window.set_monitor(window_mode, pos.0, pos.1, window_size.0, window_size.1, Some(144));
				}
			}
	});
	*/

	//Configure which window events GLFW will listen for
	window.set_key_polling(true);
	window.set_framebuffer_size_polling(true);
	window.set_mouse_button_polling(true);
	window.set_scroll_polling(true);
	window.set_cursor_pos_polling(true);

	//Initialize all OpenGL function pointers
	gl::load_with(|symbol| window.get_proc_address(symbol) as *const _);	

	//OpenGL static configuration
	unsafe {
		gl::Enable(gl::DEPTH_TEST);										//Enable depth testing
		gl::Enable(gl::CULL_FACE);										//Enable face culling
		gl::DepthFunc(gl::LESS);										//Pass the fragment with the smallest z-value.
		gl::Enable(gl::FRAMEBUFFER_SRGB); 								//Enable automatic linear->SRGB space conversion
		gl::Enable(gl::BLEND);											//Enable alpha blending
		gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);			//Set blend func to (Cs * alpha + Cd * (1.0 - alpha))
		gl::ClearColor(0.53, 0.81, 0.92, 1.0);							//Set the clear color to a pleasant blue
		gl::Enable(gl::DEBUG_OUTPUT);									//Enable verbose debug output
		gl::Enable(gl::DEBUG_OUTPUT_SYNCHRONOUS);						//Synchronously call the debug callback function
		gl::DebugMessageCallback(gl_debug_callback, ptr::null());		//Register the debug callback
		gl::DebugMessageControl(gl::DONT_CARE, gl::DONT_CARE, gl::DONT_CARE, 0, ptr::null(), gl::TRUE);
	}

	//Framebuffers used for image effects
	let ping_pong_targets = unsafe {
		let size = (window_size.0 as GLint, window_size.1 as GLint);
		let pre = RenderTarget::new(size);
		let post = RenderTarget::new(size);
		[pre, post]
	};


	//Screen filling triangle with uvs chosen such that the sampled image exactly covers the screen
	let postprocessing_vao = unsafe {
		let vs = [
			-1.0, -1.0, 0.0, 0.0,
			3.0, -1.0, 2.0, 0.0,
			-1.0, 3.0, 0.0, 2.0
		];
		glutil::create_vertex_array_object(&vs, &[0, 1, 2], &[2, 2])
	};

	//Initialize default framebuffer
	let default_framebuffer = Framebuffer {
		name: 0,
		size: (window_size.0 as GLsizei, window_size.1 as GLsizei),
		clear_flags: gl::DEPTH_BUFFER_BIT | gl::COLOR_BUFFER_BIT,
		cull_face: gl::BACK
	};

	//Compile shader programs
	let mapped_shader = unsafe { glutil::compile_program_from_files("shaders/mapped.vert", "shaders/mapped.frag") };
	let mapped_instanced_shader = unsafe { glutil::compile_program_from_files("shaders/mapped_instanced.vert", "shaders/mapped.frag") };
	let shadow_shader = unsafe { glutil::compile_program_from_files("shaders/shadow.vert", "shaders/shadow.frag") };
	let shadow_shader_instanced = unsafe { glutil::compile_program_from_files("shaders/shadow_instanced.vert", "shaders/shadow.frag") };
	let glyph_shader = unsafe { glutil::compile_program_from_files("shaders/glyph.vert", "shaders/glyph.frag") };
	let passthrough_shader = unsafe { glutil::compile_program_from_files("shaders/postprocessing.vert", "shaders/postprocessing.frag") };
	let gaussian_shader = unsafe { glutil::compile_program_from_files("shaders/postprocessing.vert", "shaders/gaussian_blur.frag") };
	let ui_shader = unsafe { glutil::compile_program_from_files("shaders/ui_button.vert", "shaders/ui_button.frag") };

	//Initialize texture caching data structure
	let mut texture_keeper = TextureKeeper::new();

	//Array of the pieces of the map
	let mut arena_pieces = Vec::new();

	//Define the floor plane
	unsafe {
		let arena_ratio = 16.0 / 9.0;
		let tex_scale = 3.0;
		let scale = 2.0;
		let vertices = [
			//Positions										Tangents					Bitangents				Normals							Texture coordinates
			-4.5*arena_ratio*scale, 0.0, -5.0*scale,		1.0, 0.0, 0.0,				0.0, 0.0, 1.0,			0.0, 1.0, 0.0,					0.0, 0.0,
			4.5*arena_ratio*scale, 0.0, -5.0*scale,			1.0, 0.0, 0.0,				0.0, 0.0, 1.0,			0.0, 1.0, 0.0,					tex_scale*arena_ratio*scale, 0.0,
			-4.5*arena_ratio*scale, 0.0, 5.0*scale,			1.0, 0.0, 0.0,				0.0, 0.0, 1.0,			0.0, 1.0, 0.0,					0.0, tex_scale*scale,
			4.5*arena_ratio*scale, 0.0, 5.0*scale,			1.0, 0.0, 0.0,				0.0, 0.0, 1.0,			0.0, 1.0, 0.0,					tex_scale*arena_ratio*scale, tex_scale*scale
		];
		let indices = [
			0u16, 1, 2,
			3, 2, 1
		];

		let piece = StaticGeometry {
			vao: glutil::create_vertex_array_object(&vertices, &indices, &[3, 3, 3, 3, 2]),
			albedo: texture_keeper.fetch_texture("bamboo_wood_semigloss", "albedo"),
			normal: texture_keeper.fetch_texture("bamboo_wood_semigloss", "normal"),
			model_matrix: glm::identity(),
			index_count: indices.len() as GLsizei
		};
		arena_pieces.push(piece);
		arena_pieces.len() - 1
	};

	//Load the tank
	let mut turret_origin = glm::zero();
	const TANK_SPEED: f32 = 2.5;
	let mut tank = match routines::load_ozymesh("models/better_tank.ozy") {
		Some(meshdata) => {
			let mut node_list = Vec::with_capacity(meshdata.names.len());
			let mut albedo_maps = Vec::with_capacity(meshdata.names.len());
			let mut normal_maps = Vec::with_capacity(meshdata.names.len());
			let mut roughness_maps = Vec::with_capacity(meshdata.names.len());
			let mut node_data = Vec::new();

			//Load node info
			for i in 0..meshdata.node_ids.len() {
				let parent = if meshdata.parent_ids[i] == 0 {
					None
				} else {
					Some(meshdata.parent_ids[i] as usize - 1)
				};

				if !node_list.contains(&(meshdata.node_ids[i] as usize - 1)) {
					node_data.push(SkeletonNode {
						transform: glm::identity(),
						parent
					});
				}
				node_list.push(meshdata.node_ids[i] as usize - 1);

				//Load albedo map
				let albedo_id = unsafe { texture_keeper.fetch_texture(&meshdata.texture_names[i], "albedo") };
				albedo_maps.push(albedo_id);

				//Load normal map
				let normal_id = unsafe { texture_keeper.fetch_texture(&meshdata.texture_names[i], "normal") };
				normal_maps.push(normal_id);

				//Load roughness map
				let roughness_id = unsafe { texture_keeper.fetch_texture(&meshdata.texture_names[i], "roughness") };
				roughness_maps.push(roughness_id);

				//Also get turret_origin
				if meshdata.names[i] == "Turret" {
					turret_origin = meshdata.origins[i];
				}
			}
			if turret_origin == glm::zero() {
				println!("No mesh named \"Turret\" when loading the tank.");
			}

			//Create the vertex array object
			let vao = unsafe { glutil::create_vertex_array_object(&meshdata.vertex_array.vertices, &meshdata.vertex_array.indices, &meshdata.vertex_array.attribute_offsets) };
			let skeleton = Skeleton {
				vao,
				node_data,
				node_list,
				geo_boundaries: meshdata.geo_boundaries,
				albedo_maps,
				normal_maps,
				roughness_maps
			};

			//Initialize the tank's gameplay data
			let tank_forward = glm::vec3(0.0, 0.0, 1.0);
			let turret_forward = glm::vec3_to_vec4(&tank_forward);
			let tank_position = glm::zero();
			Tank {
				position: tank_position,
				speed: TANK_SPEED,
				firing: false,
				forward: tank_forward,
				move_state: TankMoving::Not,
				tank_rotating: Rotating::Not,
				turret_forward,
				skeleton
			}
		}
		None => {
			panic!("Unable to load model.");
		}
	};

	//Load shell graphics
	let shell_mesh = SimpleMesh::from_ozy("models/better_shell.ozy", &mut texture_keeper);
	
	//Create GPU buffer for instanced matrices
	let shell_instanced_buffer = unsafe {
		gl::BindVertexArray(shell_mesh.vao);

		let mut b = 0;
		gl::GenBuffers(1, &mut b);
		gl::BindBuffer(gl::ARRAY_BUFFER, b);
		gl::BufferData(gl::ARRAY_BUFFER, (10000 * 16 * mem::size_of::<GLfloat>()) as GLsizeiptr, ptr::null(), gl::DYNAMIC_DRAW);

		//Attach this buffer to the shell_mesh vao
		//We have to individually bind each column of the matrix as a different vec4 vertex attribute
		for i in 0..4 {
			gl::VertexAttribPointer(5 + i,
									4,
									gl::FLOAT,
									gl::FALSE,
									(16 * mem::size_of::<GLfloat>()) as GLsizei,
									(i * 4 * mem::size_of::<GLfloat>() as GLuint) as *const c_void);
			gl::EnableVertexAttribArray(5 + i);
			gl::VertexAttribDivisor(5 + i, 1);
		}

		b
	};

	//OptionVec of all fired tank shells
	let mut shells: OptionVec<Shell> = OptionVec::new();

	//Initialize some constant transforms
	let view_from_world = glm::mat4(-1.0, 0.0, 0.0, 0.0,
								0.0, 1.0, 0.0, 0.0,
								0.0, 0.0, 1.0, 0.0,
								0.0, 0.0, 0.0, 1.0) * glm::look_at(&glm::vec3(0.0, 1.5, -1.0), &glm::vec3(0.0, 0.0, 0.0), &glm::vec3(0.0, 1.0, 0.0));
	let world_from_view = glm::affine_inverse(view_from_world);
	let ortho_size = 5.0;
	let clipping_from_view = glm::ortho(-ortho_size*aspect_ratio, ortho_size*aspect_ratio, -ortho_size, ortho_size, -ortho_size, ortho_size * 2.0);
	let clipping_from_world = clipping_from_view * view_from_world;
	let world_from_clipping = glm::affine_inverse(clipping_from_world);

	let world_space_look_direction = world_from_view * glm::vec4(0.0, 0.0, 1.0, 0.0);

	//Set up the light source
	let sun_direction = glm::normalize(&glm::vec4(1.0, 1.0, -1.0, 0.0));

	let mut last_frame_instant = Instant::now();
	let mut elapsed_time = 0.0;

	let mut world_space_mouse = world_from_clipping * glm::vec4(0.0, 0.0, 0.0, 1.0);
	let mut screen_space_mouse = glm::vec2(0.0, 0.0);
	let mut mouse_lbutton_pressed = false;
	let mut last_mouse_lbutton_pressed = false;

	let mut is_wireframe = false;

	//Each frame this is filled with Command, then drained when processed
	let mut command_buffer = Vec::new();

	//Default keybindings
	let mut key_bindings = {
		let mut map = HashMap::new();

		map.insert((InputType::Key(Key::Escape), Action::Press), Command::TogglePauseMenu);
		map.insert((InputType::Key(Key::Q), Action::Press), Command::ToggleWireframe);
		map.insert((InputType::Key(Key::W), Action::Press), Command::MoveForwards);
		map.insert((InputType::Key(Key::S), Action::Press), Command::MoveBackwards);
		map.insert((InputType::Key(Key::A), Action::Press), Command::RotateLeft);
		map.insert((InputType::Key(Key::D), Action::Press), Command::RotateRight);
		map.insert((InputType::Key(Key::GraveAccent), Action::Press), Command::ToggleDebugMenu);
		map.insert((InputType::Mouse(MouseButton::Button1), Action::Press), Command::Fire);

		//The keys here depend on the earlier bindings
		map.insert((InputType::Key(Key::W), Action::Release), Command::StopMoving);
		map.insert((InputType::Key(Key::S), Action::Release), Command::StopMoving);
		map.insert((InputType::Key(Key::A), Action::Release), Command::StopRotating);
		map.insert((InputType::Key(Key::D), Action::Release), Command::StopRotating);		

		map
	};

	//Initialize the shadow map
	let shadow_size = 8192;
	let shadow_rendertarget = unsafe {
		let mut shadow_framebuffer = 0;
		let mut shadow_texture = 0;

		gl::GenFramebuffers(1, &mut shadow_framebuffer);
		gl::GenTextures(1, &mut shadow_texture);

		//Initialize the texture
		gl::BindTexture(gl::TEXTURE_2D, shadow_texture);
		gl::TexImage2D(
			gl::TEXTURE_2D,
			0,
			gl::DEPTH_COMPONENT as GLint,
			shadow_size,
			shadow_size,
			0,
			gl::DEPTH_COMPONENT,
			gl::FLOAT,
			ptr::null()
		);
		glutil::apply_texture_parameters(&DEFAULT_TEX_PARAMS);

		gl::BindFramebuffer(gl::FRAMEBUFFER, shadow_framebuffer);
		gl::FramebufferTexture2D(
			gl::FRAMEBUFFER,
			gl::DEPTH_ATTACHMENT,
			gl::TEXTURE_2D,
			shadow_texture,
			0
		);
		gl::BindFramebuffer(gl::FRAMEBUFFER, 0);

		let framebuffer = Framebuffer {
			name: shadow_framebuffer,
			size: (shadow_size, shadow_size),
			clear_flags: gl::DEPTH_BUFFER_BIT,
			cull_face: gl::FRONT
		};

		RenderTarget {
			framebuffer,
			texture: shadow_texture
		}
	};

	let shadow_from_world = glm::mat4(-1.0, 0.0, 0.0, 0.0,
									   0.0, 1.0, 0.0, 0.0,
									   0.0, 0.0, 1.0, 0.0,
									   0.0, 0.0, 0.0, 1.0) * glm::look_at(&glm::vec4_to_vec3(&(sun_direction * 4.0)), &glm::zero(), &glm::vec3(0.0, 1.0, 0.0));
	let shadow_projection = glm::ortho(-ortho_size * 3.0, ortho_size * 3.0, -ortho_size * 3.0, ortho_size * 3.0, -ortho_size * 2.0, ortho_size * 3.0);

	let font = match FontArc::try_from_slice(include_bytes!("../fonts/Constantia.ttf")) {
		Ok(s) => { s }
		Err(e) => { panic!("{}", e) }
	};
	let mut glyph_brush = GlyphBrushBuilder::using_font(font).build();

	//Create the glyph texture
	let glyph_texture = unsafe {
		let (width, height) = glyph_brush.texture_dimensions();
		let mut tex = 0;
		gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);
		gl::GenTextures(1, &mut tex);
		gl::BindTexture(gl::TEXTURE_2D, tex);
		gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as _);
		gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as _);
		gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as _);
		gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as _);
		gl::TexImage2D(gl::TEXTURE_2D, 0, gl::RED as GLint, width as GLint, height as GLint, 0, gl::RED, gl::UNSIGNED_BYTE, ptr::null());
		tex
	};

	//Initialize glyph vao
	let mut glyph_vao = None;
	let mut glyph_count = 0;
	let clipping_from_screen = glm::mat4(
		2.0 / window_size.0 as f32, 0.0, 0.0, -1.0,
		0.0, -(2.0 / window_size.1 as f32), 0.0, 1.0,
		0.0, 0.0, 1.0, 0.0,
		0.0, 0.0, 0.0, 1.0
	);
	let mut sections = OptionVec::new();

	//Array of UI buttons
	let mut ui_buttons: OptionVec<UIButton> = OptionVec::new();
	let mut last_ui_button_count = 0;
	let mut ui_vao = None;
	let mut button_color_instanced_buffer = 0;

	//Pause menu data
	let mut pause_menu = Menu::new(
		vec!["Resume", "Settings", "Main Menu", "Exit"],
		vec![Some(Command::TogglePauseMenu), None, None, Some(Command::Quit)],
		MenuAnchor::CenterAligned(window_size.0 as f32 / 2.0, window_size.1 as f32 / 5.0)
	);

	//Debug menu data
	let mut debug_menu = Menu::new(
		vec!["Wireframe"],
		vec![Some(Command::ToggleWireframe)],
		MenuAnchor::LeftAligned(10.0, 10.0)
	);

	//Variable that determines what the update step looks like
	let mut game_state = GameState::Playing;
	let mut image_effect = ImageEffect::None;

	//Main loop
    while !window.should_close() {
		//Calculate time since the last frame started in seconds
		let delta_time = {
			let frame_instant = Instant::now();
			let dur = frame_instant.duration_since(last_frame_instant);
			last_frame_instant = frame_instant;

			//There's an underlying assumption here that frames will always take less than one second to complete
			(dur.subsec_millis() as f32 / 1000.0) + (dur.subsec_micros() as f32 / 1_000_000.0)
		};
		elapsed_time += delta_time;

		//Handle window events
        for (_, event) in glfw::flush_messages(&events) {
            match event {
				WindowEvent::Close => { window.set_should_close(true); }
				WindowEvent::Key(key, _, action, ..) => {
					if let Some(command) = key_bindings.get(&(InputType::Key(key), action)) {
						command_buffer.push(*command);
					}
				}
				WindowEvent::MouseButton(button, action, ..) => {
					if let Some(command) = key_bindings.get(&(InputType::Mouse(button), action)) {
						command_buffer.push(*command);
					}

					if action == Action::Press {
						mouse_lbutton_pressed = true;
					} else {						
						mouse_lbutton_pressed = false;
					}
				}
				WindowEvent::CursorPos(x, y) => {
					screen_space_mouse = glm::vec2(x as f32, y as f32);
					//We have to flip the y coordinate because glfw thinks (0, 0) is in the top left
					let clipping_space_mouse = glm::vec4(x as f32 / (window_size.0 as f32 / 2.0) - 1.0, -(y as f32 / (window_size.1 as f32 / 2.0) - 1.0), 0.0, 1.0);
					world_space_mouse = world_from_clipping * clipping_space_mouse;
				}
                _ => {}
            }
		}
		
		//Handle input from the UI buttons
		for i in 0..ui_buttons.len() {
			if let Some(button) = &mut ui_buttons[i] {
				if screen_space_mouse.x > button.bounds.min[0] &&
				   screen_space_mouse.x < button.bounds.max[0] &&
				   screen_space_mouse.y > button.bounds.min[1] &&
				   screen_space_mouse.y < button.bounds.max[1] {

					if last_mouse_lbutton_pressed && !mouse_lbutton_pressed {
						if let Some(command) = button.command {
							command_buffer.push(command);
						}
					}

					//Handle updating button graphics
					if button.state == ButtonState::None || (mouse_lbutton_pressed == last_mouse_lbutton_pressed) {
						let color = if mouse_lbutton_pressed {
							[0.0, 0.8, 0.0, 0.5]
						} else {
							[0.0, 0.4, 0.0, 0.5]
						};
						unsafe { update_ui_button_color_buffer(button_color_instanced_buffer, i, color); }

						button.state = ButtonState::Hovering;
					}
				} else {
					if button.state != ButtonState::None {
						let color = [0.0, 0.0, 0.0, 0.5];
						unsafe { update_ui_button_color_buffer(button_color_instanced_buffer, i, color); }

						button.state = ButtonState::None;
					}
				}
			}
		}
		
		//Process the generated commands
		for command in command_buffer.drain(0..command_buffer.len()) {
			match command {
				Command::Quit => {
					window.set_should_close(true);
				}
				Command::ToggleWireframe => {
					is_wireframe = !is_wireframe;
				}
				Command::MoveForwards => {
					tank.move_state = TankMoving::Forwards;
				}
				Command::MoveBackwards => {
					tank.move_state = TankMoving::Backwards;
				}
				Command::RotateLeft => {
					tank.tank_rotating = Rotating::Left;
				}
				Command::RotateRight => {
					tank.tank_rotating = Rotating::Right;
				}
				Command::StopMoving => {
					tank.move_state = TankMoving::Not;
				}
				Command::StopRotating => {
					tank.tank_rotating = Rotating::Not;
				}
				Command::TogglePauseMenu => {
					match game_state {
						GameState::Paused => { game_state = GameState::Resuming; }
						GameState::Playing => { game_state = GameState::Pausing; }
						_ => {}
					}
				}
				Command::Fire => { tank.firing = true; }
				Command::ToggleDebugMenu => {
					//Toggle the debug menu
					debug_menu.toggle(&mut ui_buttons, &mut sections, &mut glyph_brush);
				}
			}
		}

		//-----------Simulating-----------
		match game_state {
			GameState::Playing => {
				//Update the tank's position
				match tank.move_state {
					TankMoving::Forwards => {
						tank.position += tank.forward * -tank.speed * delta_time;
					}
					TankMoving::Backwards => {
						tank.position += tank.forward * tank.speed * delta_time;
					}
					TankMoving::Not => {}
				}

				//Update the tank's forward vector
				tank.forward = match tank.tank_rotating {
					Rotating::Left => {
						glm::vec4_to_vec3(&(glm::rotation(-glm::half_pi::<f32>() * delta_time, &glm::vec3(0.0, 1.0, 0.0)) * glm::vec3_to_vec4(&tank.forward)))
					}
					Rotating::Right => {
						glm::vec4_to_vec3(&(glm::rotation(glm::half_pi::<f32>() * delta_time, &glm::vec3(0.0, 1.0, 0.0)) * glm::vec3_to_vec4(&tank.forward)))
					}
					Rotating::Not => { tank.forward }
				};

				let tank_rotation = {
					let new_x = -glm::cross(&tank.forward, &glm::vec3(0.0, 1.0, 0.0));
					glm::mat4(new_x.x, 0.0, tank.forward.x, 0.0,
							new_x.y, 1.0, tank.forward.y, 0.0,
							new_x.z, 0.0, tank.forward.z, 0.0,
							0.0, 0.0, 0.0, 1.0
							)
				};

				tank.skeleton.node_data[0].transform = glm::translation(&tank.position) * tank_rotation;

				//Calculate turret rotation
				//Simple ray-plane intersection.
				tank.skeleton.node_data[1].transform = {
					let plane_normal = glm::vec3(0.0, 1.0, 0.0);
					let origin = tank.skeleton.node_data[0].transform * turret_origin;
					let t = glm::dot(&glm::vec4_to_vec3(&(origin - world_space_mouse)), &plane_normal) / glm::dot(&glm::vec4_to_vec3(&world_space_look_direction), &plane_normal);
					let intersection = world_space_mouse + t * world_space_look_direction;
					tank.turret_forward = glm::normalize(&(intersection - origin));
					let new_x = -glm::cross(&glm::vec4_to_vec3(&-tank.turret_forward), &glm::vec3(0.0, 1.0, 0.0));

					tank.skeleton.node_data[0].transform *
					glm::mat4(new_x.x, 0.0, -tank.turret_forward.x, 0.0,
							new_x.y, 1.0, -tank.turret_forward.y, 0.0,
							new_x.z, 0.0, -tank.turret_forward.z, 0.0,
							0.0, 0.0, 0.0, 1.0
							) * glm::affine_inverse(tank_rotation)
				};

				//Fire a shell if the mouse was clicked this frame
				if tank.firing {
					let transform = tank.skeleton.node_data[1].transform;
					let position = transform * glm::vec4(0.0, 0.0, 0.0, 1.0);
					let velocity = tank.turret_forward * 2.0;

					shells.insert(Shell {
						position,
						velocity,
						transform,
						vao: shell_mesh.vao,
						spawn_time: elapsed_time
					});
				}
				
				//Update shells
				let mut shell_transforms = vec![0.0; shells.count() * 16];
				let mut current_shell = 0;
				for i in 0..shells.len() {
					if let Some(shell) = &mut shells[i] {
						//Update position
						shell.position += shell.velocity * delta_time;

						//Update the translation part of the transform
						shell.transform[12] = shell.position.x;
						shell.transform[13] = shell.position.y;
						shell.transform[14] = shell.position.z;

						//Fill the position buffer used for instanced rendering
						for j in 0..16 {
							shell_transforms[current_shell * 16 + j] = shell.transform[j];
						}
						current_shell += 1;
					}
				}

				//Update GPU buffer storing shell transforms
				if shell_transforms.len() > 0 {
					unsafe {
						gl::BindBuffer(gl::ARRAY_BUFFER, shell_instanced_buffer);
						gl::BufferSubData(gl::ARRAY_BUFFER,
										0 as GLsizeiptr, 
										(shell_transforms.len() * mem::size_of::<GLfloat>()) as GLsizeiptr,
										&shell_transforms[0] as *const GLfloat as *const c_void
										);
					}
				}
				tank.firing = false;
			}
			GameState::Paused => {}
			GameState::Pausing => {
				//Enable the pause menu
				pause_menu.show(&mut ui_buttons, &mut sections, &mut glyph_brush);

				key_bindings.remove(&(InputType::Mouse(MouseButton::Button1), Action::Press));
				game_state = GameState::Paused;
				image_effect = ImageEffect::Blur;
			}
			GameState::Resuming => {
				//Remove the pause menu from the ui button list
				pause_menu.hide(&mut ui_buttons, &mut sections);

				//Re-enable normal controls
				key_bindings.insert((InputType::Mouse(MouseButton::Button1), Action::Press), Command::Fire);

				game_state = GameState::Playing;
				image_effect = ImageEffect::None;
			}
			_ => {}
		}
		last_mouse_lbutton_pressed = mouse_lbutton_pressed;

		//-----------Rendering-----------

		//Queue glyph_brush sections
		for sec in sections.iter() {
			if let Some(s) = sec {
				glyph_brush.queue(s);
			}			
		}

		//glyph_brush processing
		let mut glyph_result = glyph_brush.process_queued(|rect, tex_data| unsafe {
			gl::TextureSubImage2D(
				glyph_texture,
				0,
				rect.min[0] as _,
				rect.min[1] as _,
				rect.width() as _,
				rect.height() as _,
				gl::RED,
				gl::UNSIGNED_BYTE,
				tex_data.as_ptr() as _
			);
		}, glyph_vertex_transform);

		//Repeatedly resize the glyph texture until the error stops
		while let Err(BrushError::TextureTooSmall { suggested }) = glyph_result {
			let (width, height) = suggested;
			unsafe {
				gl::BindTexture(gl::TEXTURE_2D, glyph_texture);
				gl::TexImage2D(gl::TEXTURE_2D, 0, gl::RED as GLint, width as GLint, height as GLint, 0, gl::RED, gl::UNSIGNED_BYTE, ptr::null());
			}
			glyph_brush.resize_texture(width, height);
			glyph_result = glyph_brush.process_queued(|rect, tex_data| unsafe {
				gl::TextureSubImage2D(
					glyph_texture,
					0,
					rect.min[0] as _,
					rect.min[1] as _,
					rect.width() as _,
					rect.height() as _,
					gl::RED,
					gl::UNSIGNED_BYTE,
					tex_data.as_ptr() as _
				);
			}, glyph_vertex_transform);
		}
		
		//This should never fail
		match glyph_result.unwrap() {
			BrushAction::Draw(verts) => {
				if verts.len() > 0 {
					let mut vertex_buffer = Vec::with_capacity(verts.len() * 16);
					let mut index_buffer = vec![0; verts.len() * 6];
					for i in 0..verts.len() {
						for v in verts[i].iter() {
							vertex_buffer.push(*v);
						}
						
						//Fill out index buffer
						insert_index_buffer_quad(&mut index_buffer, i);
					}
					glyph_count = verts.len();

					match glyph_vao {
						Some(mut vao) => unsafe {						
							gl::DeleteVertexArrays(1, &mut vao);
							glyph_vao = Some(glutil::create_vertex_array_object(&vertex_buffer, &index_buffer, &[2, 2]));
						}
						None => unsafe {
							glyph_vao = Some(glutil::create_vertex_array_object(&vertex_buffer, &index_buffer, &[2, 2]));
						}
					}
				} else {
					if let Some(mut vao) = glyph_vao {
						unsafe { gl::DeleteVertexArrays(1, &mut vao); }
						glyph_vao = None;
					}
				}
			}
			BrushAction::ReDraw => {}
		}	

		//Create vao for the ui buttons
		if ui_buttons.count() > 0 && ui_buttons.count() != last_ui_button_count {
			unsafe { 
				let floats_per_button = 4 * 2;
				let mut vertices = vec![0.0; ui_buttons.count() * floats_per_button];
				let mut indices = vec![0u16; ui_buttons.count() * 6];

				let mut quads_added = 0;
				for i in 0..ui_buttons.len() {
					if let Some(button) = &ui_buttons[i] {
						vertices[quads_added * floats_per_button] = button.bounds.min[0];
						vertices[quads_added * floats_per_button + 1] = button.bounds.min[1];
						vertices[quads_added * floats_per_button + 2] = button.bounds.min[0];
						vertices[quads_added * floats_per_button + 3] = button.bounds.max[1];
						vertices[quads_added * floats_per_button + 4] = button.bounds.max[0];
						vertices[quads_added * floats_per_button + 5] = button.bounds.min[1];
						vertices[quads_added * floats_per_button + 6] = button.bounds.max[0];
						vertices[quads_added * floats_per_button + 7] = button.bounds.max[1];

						//Place this quad in the index buffer
						insert_index_buffer_quad(&mut indices, quads_added);
						quads_added += 1;
					}
				}

				match ui_vao {
					Some(mut vao) => {
						gl::DeleteVertexArrays(1, &mut vao);
						ui_vao = Some(glutil::create_vertex_array_object(&vertices, &indices, &[2]));
						gl::BindVertexArray(vao);
					}
					None => {
						let vao = glutil::create_vertex_array_object(&vertices, &indices, &[2]);
						ui_vao = Some(vao);
						gl::BindVertexArray(vao);
					}
				}

				//Create GPU buffer for ui button colors
				button_color_instanced_buffer = {
					let element_count = ui_buttons.count() * COLORS_PER_BUTTON;

					let mut data = vec![0.0f32; element_count * FLOATS_PER_COLOR];
					for i in 0..(data.len() / FLOATS_PER_COLOR) {
						data[i * 4] = 0.0;
						data[i * 4 + 1] = 0.0;
						data[i * 4 + 2] = 0.0;
						data[i * 4 + 3] = 0.5;
					}

					let mut b = 0;
					gl::GenBuffers(1, &mut b);
					gl::BindBuffer(gl::ARRAY_BUFFER, b);
					gl::BufferData(gl::ARRAY_BUFFER, (element_count * FLOATS_PER_COLOR * mem::size_of::<GLfloat>()) as GLsizeiptr, &data[0] as *const f32 as *const c_void, gl::DYNAMIC_DRAW);

					//Attach buffer to vao
					gl::VertexAttribPointer(1,
											4,
											gl::FLOAT,
											gl::FALSE,
											(FLOATS_PER_COLOR * mem::size_of::<GLfloat>()) as GLsizei,
											ptr::null());
					gl::EnableVertexAttribArray(1);

					b
				};
			}
		} else if ui_buttons.count() == 0 {
			if let Some(mut vao) = ui_vao {
				unsafe { gl::DeleteVertexArrays(1, &mut vao); }
				ui_vao = None;
			}
		}
		last_ui_button_count = ui_buttons.count();

		const TEXTURE_MAP_IDENTIFIERS: [&str; 4] = ["albedo_map", "normal_map", "roughness_map", "shadow_map"];
		unsafe {
			//Bind shadow framebuffer
			shadow_rendertarget.bind();

			//Bind shadow program
			gl::UseProgram(shadow_shader);

			//Render arena pieces
			for piece in arena_pieces.iter() {
				gl::BindVertexArray(piece.vao);
				glutil::bind_matrix4(shadow_shader, "mvp", &(shadow_projection * shadow_from_world * piece.model_matrix));
				gl::DrawElements(gl::TRIANGLES, piece.index_count, gl::UNSIGNED_SHORT, ptr::null());
			}

			//Render tank
			gl::BindVertexArray(tank.skeleton.vao);
			for i in 0..tank.skeleton.node_list.len() {
				let node_index = tank.skeleton.node_list[i];
				glutil::bind_matrix4(shadow_shader, "mvp", &(shadow_projection * shadow_from_world * tank.skeleton.node_data[node_index].transform));

				gl::DrawElements(gl::TRIANGLES, (tank.skeleton.geo_boundaries[i + 1] - tank.skeleton.geo_boundaries[i]) as i32, gl::UNSIGNED_SHORT, (mem::size_of::<GLushort>() * tank.skeleton.geo_boundaries[i] as usize) as *const c_void);
			}

			//Render shells
			gl::UseProgram(shadow_shader_instanced);
			gl::BindVertexArray(shell_mesh.vao);
			glutil::bind_matrix4(shadow_shader_instanced, "view_projection", &(shadow_projection * shadow_from_world));
			gl::DrawElementsInstanced(gl::TRIANGLES, shell_mesh.index_count, gl::UNSIGNED_SHORT, ptr::null(), shells.count() as GLint);

			//Main scene rendering
			ping_pong_targets[0].bind();
			
			//Set polygon fill mode
			if is_wireframe { gl::PolygonMode(gl::FRONT_AND_BACK, gl::LINE); }
			else { gl::PolygonMode(gl::FRONT_AND_BACK, gl::FILL); }

			//Bind the GLSL program
			gl::UseProgram(mapped_shader);

			//Set uniforms that are constant for the lifetime of the program
			initialize_texture_samplers(mapped_shader, &TEXTURE_MAP_IDENTIFIERS);
			glutil::bind_matrix4(mapped_shader, "shadow_matrix", &(shadow_projection * shadow_from_world));
			glutil::bind_vector4(mapped_shader, "sun_direction", &sun_direction);
			
			gl::ActiveTexture(gl::TEXTURE3);
			gl::BindTexture(gl::TEXTURE_2D, shadow_rendertarget.texture);

			//Render static pieces of the arena
			for piece in arena_pieces.iter() {
				glutil::bind_matrix4(mapped_shader, "mvp", &(clipping_from_world * piece.model_matrix));
				glutil::bind_matrix4(mapped_shader, "model_matrix", &piece.model_matrix);
				bind_texture_maps(&[piece.albedo, piece.normal]);

				gl::BindVertexArray(piece.vao);
				gl::DrawElements(gl::TRIANGLES, piece.index_count, gl::UNSIGNED_SHORT, ptr::null());
			}

			//Render the tank
			gl::BindVertexArray(tank.skeleton.vao);
			for i in 0..tank.skeleton.node_list.len() {
				let node_index = tank.skeleton.node_list[i];
				glutil::bind_matrix4(mapped_shader, "mvp", &(clipping_from_world * tank.skeleton.node_data[node_index].transform));
				glutil::bind_matrix4(mapped_shader, "model_matrix", &tank.skeleton.node_data[node_index].transform);
				bind_texture_maps(&[tank.skeleton.albedo_maps[i], tank.skeleton.normal_maps[i], tank.skeleton.roughness_maps[i]]);

				gl::DrawElements(gl::TRIANGLES, (tank.skeleton.geo_boundaries[i + 1] - tank.skeleton.geo_boundaries[i]) as i32, gl::UNSIGNED_SHORT, (mem::size_of::<GLushort>() * tank.skeleton.geo_boundaries[i] as usize) as *const c_void);
			}

			//Render tank shells
			gl::UseProgram(mapped_instanced_shader);

			//Set texture sampler values
			initialize_texture_samplers(mapped_instanced_shader, &TEXTURE_MAP_IDENTIFIERS);
			glutil::bind_matrix4(mapped_instanced_shader, "shadow_matrix", &(shadow_projection * shadow_from_world));
			glutil::bind_vector4(mapped_instanced_shader, "sun_direction", &sun_direction);

			//Bind the shadow map's data
			gl::ActiveTexture(gl::TEXTURE3);
			gl::BindTexture(gl::TEXTURE_2D, shadow_rendertarget.texture);

			//Bind the vertex array
			gl::BindVertexArray(shell_mesh.vao);

			//Bind the texture maps
			for i in 0..shell_mesh.texture_maps.len() {
				gl::ActiveTexture(gl::TEXTURE0 + i as GLenum);
				gl::BindTexture(gl::TEXTURE_2D, shell_mesh.texture_maps[i]);
			}
			glutil::bind_matrix4(mapped_instanced_shader, "view_projection", &clipping_from_world);
			gl::DrawElementsInstanced(gl::TRIANGLES, shell_mesh.index_count, gl::UNSIGNED_SHORT, ptr::null(), shells.count() as GLint);

			//Apply post-processing effects
			gl::PolygonMode(gl::FRONT_AND_BACK, gl::FILL);
			gl::BindVertexArray(postprocessing_vao);
			gl::ActiveTexture(gl::TEXTURE0);

			//Apply the active image effect
			match image_effect {
				ImageEffect::Blur => {
					let passes = 8;
					initialize_texture_samplers(passthrough_shader, &["image_texture"]);
					initialize_texture_samplers(gaussian_shader, &["image_texture"]);
	
					gl::UseProgram(gaussian_shader);
					for _ in 0..passes {
						let flags = [1, 0];
						//Do a horizontal pass followed by a vertical one. This reduces complexity from N^2 to 2N
						for i in 0..ping_pong_targets.len() {
							ping_pong_targets[i ^ 1].bind();
							gl::BindTexture(gl::TEXTURE_2D, ping_pong_targets[i].texture);
							glutil::bind_int(gaussian_shader, "horizontal", flags[i]);
							gl::DrawElements(gl::TRIANGLES, 3, gl::UNSIGNED_SHORT, ptr::null());
						}
					}
	
					//Render result to the default framebuffer
					default_framebuffer.bind();
					gl::UseProgram(passthrough_shader);
					gl::BindTexture(gl::TEXTURE_2D, ping_pong_targets[0].texture);
					gl::DrawElements(gl::TRIANGLES, 3, gl::UNSIGNED_SHORT, ptr::null());
				}
				ImageEffect::None => {
					default_framebuffer.bind();
					gl::UseProgram(passthrough_shader);
					gl::BindTexture(gl::TEXTURE_2D, ping_pong_targets[0].texture);
					gl::DrawElements(gl::TRIANGLES, 3, gl::UNSIGNED_SHORT, ptr::null());
				}
			}

			//Clear the depth buffer before rendering 2D elements
			gl::Clear(gl::DEPTH_BUFFER_BIT);

			//Render UI buttons
			if let Some(vao) = ui_vao {
				draw_ui_element(vao, ui_shader, ui_buttons.count(), &clipping_from_screen);
			}

			//Render text
			if let Some(vao) = glyph_vao {
				initialize_texture_samplers(glyph_shader, &["glyph_texture"]);
				gl::ActiveTexture(gl::TEXTURE0);
				gl::BindTexture(gl::TEXTURE_2D, glyph_texture);

				draw_ui_element(vao, glyph_shader, glyph_count, &clipping_from_screen);
			}
		}

		window.render_context().swap_buffers();
		glfw.poll_events();
    }
}
