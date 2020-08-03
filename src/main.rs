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

mod structs;

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
	for i in 0..identifiers.len() {
		glutil::bind_byte(program, identifiers[i], i as GLint);
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
		//gl::DebugMessageCallback(gl_debug_callback, ptr::null());		//Register the debug callback
		gl::DebugMessageControl(gl::DONT_CARE, gl::DONT_CARE, gl::DONT_CARE, 0, ptr::null(), gl::TRUE);
	}

	//Define the default framebuffer
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
	let shell_mesh = IndividualMesh::from_ozy("models/better_shell.ozy", &mut texture_keeper);
	
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

	//The view-projection matrix is constant
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

	let mut is_wireframe = false;

	//Each frame this is filled with commands, then drained when processed
	let mut command_buffer = Vec::new();

	//Default keybindings
	let key_bindings = {
		let mut map = HashMap::new();
		map.insert((Key::Escape, Action::Press), Commands::TogglePauseMenu);
		map.insert((Key::Q, Action::Press), Commands::ToggleWireframe);
		map.insert((Key::W, Action::Press), Commands::MoveForwards);
		map.insert((Key::S, Action::Press), Commands::MoveBackwards);
		map.insert((Key::A, Action::Press), Commands::RotateLeft);
		map.insert((Key::D, Action::Press), Commands::RotateRight);

		map.insert((Key::W, Action::Release), Commands::StopMoving);
		map.insert((Key::S, Action::Release), Commands::StopMoving);
		map.insert((Key::A, Action::Release), Commands::StopRotating);
		map.insert((Key::D, Action::Release), Commands::StopRotating);
		map
	};

	//Initialize the shadow map
	let shadow_size = 4096;
	let (shadow_framebuffer, shadow_texture) = unsafe {
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
		gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as i32);
		gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as i32);
		gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::REPEAT as i32);
		gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::REPEAT as i32);

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
		(framebuffer, shadow_texture)
	};

	let shadow_from_world = glm::mat4(-1.0, 0.0, 0.0, 0.0,
									   0.0, 1.0, 0.0, 0.0,
									   0.0, 0.0, 1.0, 0.0,
									   0.0, 0.0, 0.0, 1.0) * glm::look_at(&glm::vec4_to_vec3(&(sun_direction * 4.0)), &glm::zero(), &glm::vec3(0.0, 1.0, 0.0));
	let shadow_projection = glm::ortho(-ortho_size * 2.0, ortho_size * 2.0, -ortho_size * 2.0, ortho_size * 2.0, -ortho_size, ortho_size * 3.0);

	let font = match FontArc::try_from_slice(include_bytes!("../fonts/Constantia.ttf")) {
		Ok(s) => { s }
		Err(e) => { panic!("{}", e) }
	};
	let mut glyph_brush = GlyphBrushBuilder::using_font(font).initial_cache_size((512, 512)).build();

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

	let mut section = Section::new();
	section.screen_position = (50.0, 20.0);
	let mut text = Text::new("First line of text").with_color([1.0, 1.0, 1.0, 1.0]);
	text.scale = PxScale::from(24.0);
	sections.insert(section.add_text(text));

	let mut section = Section::new();
	section.screen_position = (50.0, 80.0);
	let mut text = Text::new("Second line of text").with_color([1.0, 1.0, 1.0, 1.0]);
	text.scale = PxScale::from(24.0);
	sections.insert(section.add_text(text));

	let mut game_state = GameState::Playing;

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
					match key_bindings.get(&(key, action)) {
						Some(command) => { command_buffer.push(command); }
						None => {  }
					}
				}
				WindowEvent::CursorPos(x, y) => {
					//We have to flip the y coordinate because glfw thinks (0, 0) is in the top left
					let clipping_space_mouse = glm::vec4(x as f32 / (window_size.0 as f32 / 2.0) - 1.0, -(y as f32 / (window_size.1 as f32 / 2.0) - 1.0), 0.0, 1.0);
					world_space_mouse = world_from_clipping * clipping_space_mouse;
				}
				WindowEvent::MouseButton(MouseButton::Button1, Action::Press, ..) => { tank.firing = true; }
                _ => {}
            }
        }
		
		//Process the generated Commands
		for command in command_buffer.drain(0..command_buffer.len()) {
			match command {
				Commands::Quit => {
					window.set_should_close(true);
				}
				Commands::ToggleWireframe => unsafe {
					is_wireframe = !is_wireframe;
					if is_wireframe { gl::PolygonMode(gl::FRONT_AND_BACK, gl::LINE); }
					else { gl::PolygonMode(gl::FRONT_AND_BACK, gl::FILL); }
				}
				Commands::MoveForwards => {
					tank.move_state = TankMoving::Forwards;
				}
				Commands::MoveBackwards => {
					tank.move_state = TankMoving::Backwards;
				}
				Commands::RotateLeft => {
					tank.tank_rotating = Rotating::Left;
				}
				Commands::RotateRight => {
					tank.tank_rotating = Rotating::Right;
				}
				Commands::StopMoving => {
					tank.move_state = TankMoving::Not;
				}
				Commands::StopRotating => {
					tank.tank_rotating = Rotating::Not;
				}
				Commands::TogglePauseMenu => {
					game_state = GameState::Paused;
				}
				Commands::ToggleFreecam => {

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
			_ => {}
		}


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

		//Resize the glyph texture if it's too small
		if let Err(BrushError::TextureTooSmall { suggested }) = glyph_result {
			println!("Resizing glyph_texture {:?}", suggested);
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
				let mut vertex_buffer = Vec::with_capacity(verts.len() * 16);
				let mut index_buffer = vec![0; verts.len() * 6];
				for vert in verts.iter() {
					for v in vert {
						vertex_buffer.push(*v);
					}
				}
				glyph_count = verts.len();

				for i in 0..verts.len() {
					index_buffer[i * 6] = 4 * i as u16;
					index_buffer[i * 6 + 1] = index_buffer[i * 6] + 1;
					index_buffer[i * 6 + 2] = index_buffer[i * 6] + 2;
					index_buffer[i * 6 + 3] = index_buffer[i * 6] + 3;
					index_buffer[i * 6 + 4] = index_buffer[i * 6] + 2;
					index_buffer[i * 6 + 5] = index_buffer[i * 6] + 1;
				}

				match glyph_vao {
					Some(mut vao) => unsafe {						
						gl::DeleteVertexArrays(1, &mut vao);
						glyph_vao = Some(glutil::create_vertex_array_object(&vertex_buffer, &index_buffer, &[2, 2]));
					}
					None => unsafe {
						glyph_vao = Some(glutil::create_vertex_array_object(&vertex_buffer, &index_buffer, &[2, 2]));
					}
				}
				println!("Rendered new text");
			}
			BrushAction::ReDraw => {}
		}

		const TEXTURE_MAP_IDENTIFIERS: [&str; 4] = ["albedo_map", "normal_map", "roughness_map", "shadow_map"];
		unsafe {
			//Bind shadow framebuffer
			shadow_framebuffer.bind();

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
			default_framebuffer.bind();

			//Bind the GLSL program
			gl::UseProgram(mapped_shader);

			//Set uniforms that are constant for the lifetime of the program
			initialize_texture_samplers(mapped_shader, &TEXTURE_MAP_IDENTIFIERS);
			glutil::bind_matrix4(mapped_shader, "shadow_matrix", &(shadow_projection * shadow_from_world));
			glutil::bind_vector4(mapped_shader, "sun_direction", &sun_direction);
			gl::ActiveTexture(gl::TEXTURE3);
			gl::BindTexture(gl::TEXTURE_2D, shadow_texture);

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
			gl::BindTexture(gl::TEXTURE_2D, shadow_texture);

			//Bind the vertex array
			gl::BindVertexArray(shell_mesh.vao);

			//Bind the texture maps
			for i in 0..shell_mesh.texture_maps.len() {
				gl::ActiveTexture(gl::TEXTURE0 + i as GLenum);
				gl::BindTexture(gl::TEXTURE_2D, shell_mesh.texture_maps[i]);
			}
			glutil::bind_matrix4(mapped_instanced_shader, "view_projection", &clipping_from_world);
			gl::DrawElementsInstanced(gl::TRIANGLES, shell_mesh.index_count, gl::UNSIGNED_SHORT, ptr::null(), shells.count() as GLint);

			//Clear the depth buffer before rendering 2D elements
			gl::Clear(gl::DEPTH_BUFFER_BIT);

			//Render text
			if let Some(vao) = glyph_vao {
				gl::UseProgram(glyph_shader);
				glutil::bind_matrix4(glyph_shader, "clipping_from_screen", &clipping_from_screen);
				initialize_texture_samplers(glyph_shader, &["glyph_texture"]);
				gl::ActiveTexture(gl::TEXTURE0);
				gl::BindTexture(gl::TEXTURE_2D, glyph_texture);
				gl::BindVertexArray(vao);
				gl::DrawElements(gl::TRIANGLES, 6 * glyph_count as GLint, gl::UNSIGNED_SHORT, ptr::null());
			}
		}

		window.render_context().swap_buffers();
		glfw.poll_events();
    }
}
