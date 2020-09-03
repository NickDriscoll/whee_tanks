extern crate nalgebra_glm as glm;
use std::{mem, ptr};
use std::collections::HashMap;
use std::fs::{File};
use std::io::BufReader;
use std::os::raw::c_void;
use std::time::Instant;
use glfw::{Action, Context, Key, MouseButton, WindowEvent, WindowMode};
use gl::types::*;
use glyph_brush::{ab_glyph::{FontArc, PxScale}, GlyphBrushBuilder, GlyphCruncher, Section, Text};
use rodio::{Sink};
use ozy_engine::{glutil, routines};
use ozy_engine::structs::OptionVec;
use crate::structs::*;
use crate::input::{Command, Input, InputKind};
use crate::ui::{Menu, UIAnchor, UIState};

mod structs;
mod input;
mod ui;

const WINDOWED_SIZE: (u32, u32) = (1920, 1080);
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

#[cfg(gloutput)]
extern "system" fn gl_debug_callback(source: GLenum, gltype: GLenum, id: GLuint, severity: GLenum, length: GLsizei, message: *const GLchar, _: *mut c_void) {
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

pub unsafe fn draw_ui_elements(vao: GLuint, shader: GLuint, count: usize, clipping_from_screen: &glm::TMat4<f32>) {
    gl::UseProgram(shader);
	glutil::bind_matrix4(shader, "clipping_from_screen", &clipping_from_screen);
	gl::BindVertexArray(vao);
	gl::DrawElements(gl::TRIANGLES, 6 * count as GLint, gl::UNSIGNED_SHORT, ptr::null());
}

pub fn submit_input_command(input: &Input, command_buffer: &mut Vec<Command>, bindings: &HashMap<Input, Command>) {	
	if let Some(command) = bindings.get(input) {
		command_buffer.push(*command);
	}
}

fn main() {
	let mut is_fullscreen = false;
	let game_title = "Whee! Tanks! for ipad";

	//Initialize some constant transforms
	let view_from_world = glm::mat4(-1.0, 0.0, 0.0, 0.0,
									 0.0, 1.0, 0.0, 0.0,
									 0.0, 0.0, 1.0, 0.0,
									 0.0, 0.0, 0.0, 1.0) * glm::look_at(&glm::vec3(0.0, 1.5, -1.0), &glm::vec3(0.0, 0.0, 0.0), &glm::vec3(0.0, 1.0, 0.0));
	let world_from_view = glm::affine_inverse(view_from_world);
	let world_space_look_direction = world_from_view * glm::vec4(0.0, 0.0, 1.0, 0.0);

	//Variables that depend on screen size
	let screen_size = (1920, 1080);

	//Init glfw
	let mut glfw = match glfw::init(glfw::FAIL_ON_ERRORS) {
		Ok(g) => { g }
		Err(e) => {	panic!("GLFW init error: {}", e); }
	};

	glfw.window_hint(glfw::WindowHint::ContextVersion(4, 3));
	glfw.window_hint(glfw::WindowHint::OpenGlProfile(glfw::OpenGlProfileHint::Core));
	glfw.window_hint(glfw::WindowHint::OpenGlDebugContext(true));

	//Create window
    let (mut window, events) = glfw.create_window(screen_size.0, screen_size.1, game_title, WindowMode::Windowed).unwrap();

	//Make the window non-resizable
	window.set_resizable(false);

	//Configure which window events GLFW will listen for
	window.set_key_polling(true);
	window.set_framebuffer_size_polling(true);
	window.set_mouse_button_polling(true);
	window.set_scroll_polling(true);
	window.set_cursor_pos_polling(true);

	//Initialize all OpenGL function pointers
	gl::load_with(|symbol| window.get_proc_address(symbol) as *const _);

	//Struct of state that depends on screen size
	let mut screen_state = ScreenState::new(screen_size, &view_from_world);

	//OpenGL static configuration
	unsafe {
		gl::Enable(gl::CULL_FACE);										//Enable face culling
		gl::DepthFunc(gl::LESS);										//Pass the fragment with the smallest z-value.
		gl::Enable(gl::FRAMEBUFFER_SRGB); 								//Enable automatic linear->SRGB space conversion
		gl::Enable(gl::BLEND);											//Enable alpha blending
		gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);			//Set blend func to (Cs * alpha + Cd * (1.0 - alpha))
		gl::ClearColor(0.53, 0.81, 0.92, 1.0);							//Set the clear color to a pleasant blue
		gl::Enable(gl::DEBUG_OUTPUT);									//Enable verbose debug output
		gl::Enable(gl::DEBUG_OUTPUT_SYNCHRONOUS);						//Synchronously call the debug callback function

		#[cfg(gloutput)]
		{
			gl::DebugMessageCallback(gl_debug_callback, ptr::null());		//Register the debug callback
			gl::DebugMessageControl(gl::DONT_CARE, gl::DONT_CARE, gl::DONT_CARE, 0, ptr::null(), gl::TRUE);
		}
	}

	//Screen filling triangle with uvs chosen such that the sampled image exactly covers the screen
	let postprocessing_vao = unsafe {
		let vs = [
			-1.0, -1.0, 0.0, 0.0,
			3.0, -1.0, 2.0, 0.0,
			-1.0, 3.0, 0.0, 2.0
		];
		glutil::create_vertex_array_object(&vs, &[0, 1, 2], &[2, 2])
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
	let edit_shader = unsafe { glutil::compile_program_from_files("shaders/mapped.vert", "shaders/edit.frag") };

	//Initialize texture caching data structure
	let mut texture_keeper = TextureKeeper::new();

	//Array of the pieces of the map
	let mut arena_pieces = Vec::new();

	//Define the floor plane
	let arena_ratio = 16.0 / 9.0;
	let scale = 2.0;
	let floor_half_size = (4.5*arena_ratio*scale, 5.0*scale);
	unsafe {
		let tex_scale = 3.0;
		let vertices = [
			//Positions										Tangents					Bitangents				Normals							Texture coordinates
			-floor_half_size.0, 0.0, -floor_half_size.1,		1.0, 0.0, 0.0,				0.0, 0.0, 1.0,			0.0, 1.0, 0.0,					0.0, 0.0,
			floor_half_size.0, 0.0, -floor_half_size.1,			1.0, 0.0, 0.0,				0.0, 0.0, 1.0,			0.0, 1.0, 0.0,					tex_scale*arena_ratio*scale, 0.0,
			-floor_half_size.0, 0.0, floor_half_size.1,			1.0, 0.0, 0.0,				0.0, 0.0, 1.0,			0.0, 1.0, 0.0,					0.0, tex_scale*scale,
			floor_half_size.0, 0.0, floor_half_size.1,			1.0, 0.0, 0.0,				0.0, 0.0, 1.0,			0.0, 1.0, 0.0,					tex_scale*arena_ratio*scale, tex_scale*scale
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

	//Array of all tanks
	let mut tanks: OptionVec<Tank> = OptionVec::new();

	//Load the tank skeleton
	let mut turret_origin = glm::zero();
	let tank_skeleton = match routines::load_ozymesh("models/better_tank.ozy") {
		Some(meshdata) => {
			let mut node_list = Vec::with_capacity(meshdata.names.len());
			let mut albedo_maps = Vec::with_capacity(meshdata.names.len());
			let mut normal_maps = Vec::with_capacity(meshdata.names.len());
			let mut roughness_maps = Vec::with_capacity(meshdata.names.len());
			let mut bones = Vec::new();

			//Load node info
			for i in 0..meshdata.node_ids.len() {
				let parent = if meshdata.parent_ids[i] == 0 {
					None
				} else {
					Some(meshdata.parent_ids[i] as usize - 1)
				};

				if !node_list.contains(&(meshdata.node_ids[i] as usize - 1)) {
					bones.push(Bone {
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
			Skeleton {
				vao,
				node_list,
				geo_boundaries: meshdata.geo_boundaries,
				albedo_maps,
				normal_maps,
				roughness_maps,
				bones
			}
		}
		None => {
			panic!("Unable to load model.");
		}
	};
	let mut player_tank_id = 0;

	//OptionVec of all fired tank shells
	let mut shells: OptionVec<Shell> = OptionVec::new();

	//Load shell graphics
	let shell_mesh = SimpleMesh::from_ozy("models/better_shell.ozy", &mut texture_keeper);
	
	//Create GPU buffer for instanced matrices
	let maximum_live_shells = 10000;
	let shell_instanced_transforms = unsafe {
		gl::BindVertexArray(shell_mesh.vao);

		let mut b = 0;
		gl::GenBuffers(1, &mut b);
		gl::BindBuffer(gl::ARRAY_BUFFER, b);
		gl::BufferData(gl::ARRAY_BUFFER, (maximum_live_shells * 16 * mem::size_of::<GLfloat>()) as GLsizeiptr, ptr::null(), gl::DYNAMIC_DRAW);

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

	//Set up the light source
	let sun_direction = glm::normalize(&glm::vec4(1.0, 1.0, -1.0, 0.0));

	//Frame timing data
	let mut last_frame_instant = Instant::now();
	let mut elapsed_time = 0.0;

	let mut is_wireframe = false;

	//Each frame this is filled with Commands, then drained when processed
	let mut command_buffer = Vec::new();

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

	let ortho_size = 5.0;
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

	//Mouse state
	let mut world_space_mouse = screen_state.world_from_clipping * glm::vec4(0.0, 0.0, 0.0, 1.0);
	let mut screen_space_mouse = glm::vec2(0.0, 0.0);
	let mut mouse_lbutton_pressed = false;
	let mut last_mouse_lbutton_pressed = false;

	//Hardcoded menu indices
	let main_menu_index = 0;
	let pause_menu_index = 1;
	let dev_menu_index = 2;
	let settings_menu_index = 3;

	//Hardcoded menu chain indices
	let main_menu_chain_index = 0;

	//Data structure of all UI state
	let mut ui_state = {
		let mut menus = Vec::new();
		
		//Main Menu data
		let menu = Menu::new(
			vec![
				("Singleplayer", Some(Command::StartPlaying)),
				("Multiplayer", None),
				("Settings", Some(Command::AppendToMenuChain(main_menu_chain_index, settings_menu_index))),
				("Exit", Some(Command::Quit)),
			],
			UIAnchor::CenterAligned(screen_state.window_size.0 as f32 / 2.0, screen_state.window_size.1 as f32 / 3.0)
		);
		menus.push(menu);

		//Pause menu data
		let menu = Menu::new(
			vec![
				("Resume", Some(Command::UnPauseGame)),
				("Settings", Some(Command::AppendToMenuChain(main_menu_chain_index, settings_menu_index))),
				("Main Menu", Some(Command::ReturnToMainMenu)),
				("Exit", Some(Command::Quit)),
			],
			UIAnchor::CenterAligned(screen_state.window_size.0 as f32 / 2.0, screen_state.window_size.1 as f32 / 3.0)
		);
		menus.push(menu);

		//Dev menu
		let menu = Menu::new(
			vec![
				("Toggle collision volumes", None)
			],
			UIAnchor::LeftAligned(20.0, 20.0)
		);
		menus.push(menu);

		//Settings menu data
		let menu = Menu::new(
			vec![
				("Toggle fullscreen", Some(Command::ToggleFullScreen)),
				("Back", Some(Command::MenuChainRollback(main_menu_chain_index))),
			],
			UIAnchor::CenterAligned(screen_state.window_size.0 as f32 / 2.0, screen_state.window_size.1 as f32 / 3.0)
		);
		menus.push(menu);

		let mut state = UIState::new(menus, &mut glyph_brush);
		state.create_menu_chain(main_menu_index);
		state
	};

	//Title text
	let mut title_section = {
		let font_size = 72.0;
		let section = Section::new();
		let mut text = Text::new(game_title).with_color([1.0, 1.0, 1.0, 1.0]);
		text.scale = PxScale::from(font_size);
		let mut section = section.add_text(text);
	
		let bounding_box = ui_state.internals.glyph_brush.glyph_bounds(&section).unwrap();
		section.screen_position = (
			screen_state.window_size.0 as f32 / 2.0 - bounding_box.width() / 2.0,
			40.0
		);
		section
	};
	let mut title_section_index = ui_state.add_section(title_section.clone());

	//Background music data
	let bgm_path = "music/factory.mp3";
	let bgm_volume = 0.5;
	let bgm_sink = match rodio::default_output_device() {
		Some(device) => {
			let sink = Sink::new(&device);
			Some(sink)
		}
		None => { None }
	};

	//Initialize game state
	let mut game_state = {
		let mut input_maps = HashMap::new();

		//Default key bindings for now
		let key_bindings = {
			let mut map = HashMap::new();

			map.insert((InputKind::Key(Key::Escape), Action::Press), Command::PauseGame);
			map.insert((InputKind::Key(Key::Q), Action::Press), Command::ToggleWireframe);
			map.insert((InputKind::Key(Key::W), Action::Press), Command::MoveForwards);
			map.insert((InputKind::Key(Key::S), Action::Press), Command::MoveBackwards);
			map.insert((InputKind::Key(Key::A), Action::Press), Command::RotateLeft);
			map.insert((InputKind::Key(Key::D), Action::Press), Command::RotateRight);
			map.insert((InputKind::Key(Key::GraveAccent), Action::Press), Command::ToggleMenu(dev_menu_index));
			map.insert((InputKind::Mouse(MouseButton::Button1), Action::Press), Command::Fire);

			//The keys here depend on the earlier bindings
			map.insert((InputKind::Key(Key::W), Action::Release), Command::StopMoveForwards);
			map.insert((InputKind::Key(Key::S), Action::Release), Command::StopMoveBackwards);
			map.insert((InputKind::Key(Key::A), Action::Release), Command::StopRotateLeft);
			map.insert((InputKind::Key(Key::D), Action::Release), Command::StopRotateRight);		

			map
		};
		input_maps.insert(GameStateKind::Playing, key_bindings);

		//Pause menu keybindings
		let key_bindings = {
			let mut map = HashMap::new();

			map.insert((InputKind::Key(Key::Escape), Action::Press), Command::UnPauseGame);	
			map.insert((InputKind::Key(Key::GraveAccent), Action::Press), Command::ToggleMenu(dev_menu_index));

			map
		};
		input_maps.insert(GameStateKind::Paused, key_bindings);

		GameState::new(GameStateKind::MainMenu, input_maps)
	};

	//Effect to use during the image effect step
	let mut image_effect = ImageEffect::None;

	//Main loop
    while !window.should_close() {
		//Calculate time since the last frame started in seconds
		let delta_time = {
			let frame_instant = Instant::now();
			let dur = frame_instant.duration_since(last_frame_instant);
			last_frame_instant = frame_instant;
			dur.as_secs_f32()
		};

		//Handle window events
		let key_bindings = game_state.get_input_map();				//Retrieve the input map to be used this frame based on the current gamestate
        for (_, event) in glfw::flush_messages(&events) {
            match event {
				WindowEvent::Close => { window.set_should_close(true); }
				WindowEvent::Key(key, _, action, ..) => {
					submit_input_command(&(InputKind::Key(key), action), &mut command_buffer, &key_bindings);
				}
				WindowEvent::MouseButton(button, action, ..) => {
					submit_input_command(&(InputKind::Mouse(button), action), &mut command_buffer, &key_bindings);

					if button == MouseButton::Button1 {
						mouse_lbutton_pressed = action == Action::Press;
					}
				}
				WindowEvent::CursorPos(x, y) => {
					screen_space_mouse = glm::vec2(x as f32, y as f32);
					//We have to flip the y coordinate because glfw thinks (0, 0) is in the top left
					let clipping_space_mouse = glm::vec4(x as f32 / (screen_state.window_size.0 as f32 / 2.0) - 1.0, -(y as f32 / (screen_state.window_size.1 as f32 / 2.0) - 1.0), 0.0, 1.0);
					world_space_mouse = screen_state.world_from_clipping * clipping_space_mouse;
				}
                _ => {}
            }
		}
		
		//Handle input from the UI buttons
		ui_state.update_buttons(screen_space_mouse, mouse_lbutton_pressed, last_mouse_lbutton_pressed, &mut command_buffer);
		
		//Process the generated commands
		for command in command_buffer.drain(0..command_buffer.len()) {
			match command {
				Command::Quit => { window.set_should_close(true); }
				Command::ToggleWireframe => { is_wireframe = !is_wireframe; }
				Command::ToggleFullScreen => {
					if is_fullscreen {
						screen_state = ScreenState::new(WINDOWED_SIZE, &view_from_world);
						window.set_monitor(WindowMode::Windowed, 200, 200, screen_state.window_size.0, screen_state.window_size.1, Some(144));
					} else {
						glfw.with_primary_monitor_mut(|_, opt_monitor| {
							if let Some(monitor) = opt_monitor {
								let pos = monitor.get_pos();
								if let Some(mode) = monitor.get_video_mode() {
									screen_state = ScreenState::new((mode.width, mode.height), &view_from_world);
									window.set_monitor(WindowMode::FullScreen(monitor), pos.0, pos.1, screen_state.window_size.0, screen_state.window_size.1, Some(144));
								}
							}
						});
					}
					is_fullscreen = !is_fullscreen;

					//Update the UI elements that depend on screen size
					ui_state.update_screen_size(screen_state.window_size);
					let bounding_box = ui_state.internals.glyph_brush.glyph_bounds(&title_section).unwrap();
					title_section.screen_position = (
						screen_state.window_size.0 as f32 / 2.0 - bounding_box.width() / 2.0,
						40.0
					);
					ui_state.delete_section(title_section_index);
					title_section_index = ui_state.add_section(title_section.clone());
				}
				Command::MoveForwards => {
					if let Some(tank) = tanks.get_mut_element(player_tank_id) {
						tank.speed -= Tank::SPEED;
					}
				}
				Command::MoveBackwards => {
					if let Some(tank) = tanks.get_mut_element(player_tank_id) {
						tank.speed += Tank::SPEED;
					}
				}
				Command::StopMoveForwards => {
					if let Some(tank) = tanks.get_mut_element(player_tank_id) {
						tank.speed += Tank::SPEED;
					}
				}
				Command::StopMoveBackwards => {
					if let Some(tank) = tanks.get_mut_element(player_tank_id) {
						tank.speed -= Tank::SPEED;
					}
				}
				Command::RotateLeft => {
					if let Some(tank) = tanks.get_mut_element(player_tank_id) {
						tank.rotating -= Tank::ROTATION_SPEED;
					}
				}
				Command::RotateRight => {
					if let Some(tank) = tanks.get_mut_element(player_tank_id) {
						tank.rotating += Tank::ROTATION_SPEED;
					}
				}
				Command::StopRotateLeft => {
					if let Some(tank) = tanks.get_mut_element(player_tank_id) {
						tank.rotating += Tank::ROTATION_SPEED;
					}
				}
				Command::StopRotateRight => {
					if let Some(tank) = tanks.get_mut_element(player_tank_id) {
						tank.rotating -= Tank::ROTATION_SPEED;
					}
				}
				Command::PauseGame => {
					if let Some(tank) = tanks.get_mut_element(player_tank_id) {
						tank.speed = 0.0;
						tank.rotating = 0.0;
					}
					
					//Enable the pause menu
					ui_state.append_to_chain(main_menu_chain_index, pause_menu_index);
					title_section_index = ui_state.add_section(title_section.clone());
	
					game_state.kind = GameStateKind::Paused;
					image_effect = ImageEffect::Blur;
					if let Some(sink) = &bgm_sink {
						sink.set_volume(bgm_volume * 0.25);
					}
				}
				Command::UnPauseGame => {
					//Hide UI
					ui_state.reset();
					game_state.kind = GameStateKind::Playing;							
					image_effect = ImageEffect::None;
					if let Some(sink) = &bgm_sink {
						sink.set_volume(bgm_volume);
					}
				}				
				Command::Fire => {
					if let Some(tank) = tanks.get_mut_element(player_tank_id) {
						tank.firing = true;
					}
				}
				Command::StartPlaying => {
					ui_state.reset();
					game_state.kind = GameStateKind::Playing;
					image_effect = ImageEffect::None;
					elapsed_time = 0.0;

					//Initialize the player's tank
					player_tank_id = {
						let tank_forward = glm::vec3(-1.0, 0.0, 0.0);
						let tank_position = glm::vec3(-4.5, 0.0, 0.0);
						let tank = Tank::new(tank_position, tank_forward, &tank_skeleton, Brain::PlayerInput);
						tanks.insert(tank)
					};

					let tank_forward = glm::vec3(1.0, 0.0, 0.0);
					let tank_position = glm::vec3(4.5, 0.0, 0.0);
					let tank = Tank::new(tank_position, tank_forward, &tank_skeleton, Brain::DumbAI);
					tanks.insert(tank);

					//Start the music
					if let Some(sink) = &bgm_sink {
						if sink.empty() {
							let file = File::open(bgm_path).unwrap();
							let source = rodio::Decoder::new(BufReader::new(file)).unwrap();
							sink.append(source);
							sink.set_volume(bgm_volume);
						} else {
							sink.play();
							sink.set_volume(bgm_volume);
						}
					}
				}
				Command::ReturnToMainMenu => {
					//Reset game state
					tanks.clear();
					shells.clear();

					//Reset UI state
					ui_state.reset();
					title_section_index = ui_state.add_section(title_section.clone());
					ui_state.append_to_chain(main_menu_chain_index, main_menu_index);

					image_effect = ImageEffect::None;
					game_state.kind = GameStateKind::MainMenu;

					//Stop the music
					if let Some(sink) = &bgm_sink {
						sink.stop();
					}
				}
				Command::ToggleMenu(index) => {
					ui_state.toggle_menu(index);
				}
				Command::AppendToMenuChain(chain, dst) => {
					ui_state.append_to_chain(chain, dst);
				}
				Command::MenuChainRollback(chain) => {
					ui_state.rollback_chain(chain);
				}
			}
		}

		//-----------Simulating-----------
		match game_state.kind {
			GameStateKind::Playing => {
				elapsed_time += delta_time;

				let mut player_origin = glm::vec4(0.0, 0.0, 0.0, 1.0);
				if let Some(tank) = &tanks[player_tank_id] {
					player_origin = tank.bones[0].transform * turret_origin;
				}

				//Update the tanks				
				for j in 0..tanks.len() {
					if let Some(tank) = tanks.get_mut_element(j) {
						//Update the tank's forward vector
						tank.forward = glm::vec4_to_vec3(&(glm::rotation(tank.rotating * delta_time, &glm::vec3(0.0, 1.0, 0.0)) * glm::vec3_to_vec4(&tank.forward)));

						//Update the tank's position
						tank.position += tank.forward * tank.speed * delta_time;

						tank.rotation = {
							let new_x = -glm::cross(&tank.forward, &glm::vec3(0.0, 1.0, 0.0));
							glm::mat4(new_x.x, 0.0, tank.forward.x, 0.0,
									new_x.y, 1.0, tank.forward.y, 0.0,
									new_x.z, 0.0, tank.forward.z, 0.0,
									0.0, 0.0, 0.0, 1.0
									)
						};

						tank.bones[0].transform = glm::translation(&tank.position) * tank.rotation;
						
						tank.turret_origin = tank.bones[0].transform * turret_origin;
						let aim_target;
						match &mut tank.brain {
							Brain::PlayerInput => {
								//Simple ray-plane intersection.
								let plane_normal = glm::vec3(0.0, 1.0, 0.0);
								let t = glm::dot(&glm::vec4_to_vec3(&(tank.turret_origin - world_space_mouse)), &plane_normal) / glm::dot(&glm::vec4_to_vec3(&world_space_look_direction), &plane_normal);
								let intersection = world_space_mouse + t * world_space_look_direction;

								//Point the turret at the mouse cursor
								aim_target = intersection;
							}
							Brain::DumbAI => {
								//Point at player
								aim_target = player_origin;

								//Set firing flag
								tank.firing = true;
							}
						}
						tank.aim_turret(&aim_target);

						//Fire a shell if the tank's firing flag is set and if the tank is not in cooldown
						if tank.firing {
							tank.firing = false;
							let timer_expired = elapsed_time > tank.last_shot_time + Tank::SHOT_COOLDOWN;			//Has this tank cooled down from its last shot?
							let shell_buffer_has_room = shells.count() < maximum_live_shells;						//Is the shell buffer completely full?

							//If all conditions are met, fire a shell
							if timer_expired && shell_buffer_has_room {
								tank.last_shot_time = elapsed_time;
	
								let transform = tank.bones[1].transform;
								let position = transform * glm::vec4(0.0, 0.0, 0.0, 1.0);
								let velocity = tank.turret_forward * Shell::VELOCITY;
	
								shells.insert(Shell {
									position,
									velocity,
									transform,
									spawn_time: elapsed_time as f32
								});								
							}
						}
					}
				}

				//Update shells
				let floats_per_transform = 16;
				let mut shell_transforms = vec![0.0; shells.count() * floats_per_transform];
				let mut current_shell = 0;
				for i in 0..shells.len() {
					if let Some(shell) = shells.get_mut_element(i) {
						//Check if the shell needs to be de-spawned
						if elapsed_time > shell.spawn_time + Shell::LIFETIME {
							shells.delete(i);
							continue;
						}

						//Update position
						shell.position += shell.velocity * delta_time;

						//Update the translation part of the transform
						shell.transform[12] = shell.position.x;
						shell.transform[13] = shell.position.y;
						shell.transform[14] = shell.position.z;

						//Fill the transform buffer used for instanced rendering
						for j in 0..floats_per_transform {
							shell_transforms[current_shell * floats_per_transform + j] = shell.transform[j];
						}
						current_shell += 1;
					}
				}

				//Update GPU buffer storing shell transforms
				if shell_transforms.len() > 0 {
					unsafe {
						gl::BindBuffer(gl::ARRAY_BUFFER, shell_instanced_transforms);
						gl::BufferSubData(gl::ARRAY_BUFFER,
										0 as GLsizeiptr, 
										(shell_transforms.len() * mem::size_of::<GLfloat>()) as GLsizeiptr,
										&shell_transforms[0] as *const GLfloat as *const c_void
										);
					}
				}
			}
			GameStateKind::MainMenu => {}
			GameStateKind::Paused => {}
		}
		last_mouse_lbutton_pressed = mouse_lbutton_pressed;

		//-----------CPU-side UI element rendering-----------
		ui_state.synchronize();

		//The names of the texture maps in shaders/mapped.frag
		const TEXTURE_MAP_IDENTIFIERS: [&str; 4] = ["albedo_map", "normal_map", "roughness_map", "shadow_map"];

		//Rendering
		unsafe {
			//Enable depth testing for 3D scene drawing
			gl::Enable(gl::DEPTH_TEST);

			//-----------Shadow map rendering-----------

			//Bind shadowmap fbo
			shadow_rendertarget.bind();

			//Bind shadow program
			gl::UseProgram(shadow_shader);

			//Render arena pieces
			for piece in arena_pieces.iter() {
				gl::BindVertexArray(piece.vao);
				glutil::bind_matrix4(shadow_shader, "mvp", &(shadow_projection * shadow_from_world * piece.model_matrix));
				gl::DrawElements(gl::TRIANGLES, piece.index_count, gl::UNSIGNED_SHORT, ptr::null());
			}

			//Render tanks
			gl::BindVertexArray(tank_skeleton.vao);
			for i in 0..tanks.len() {
				if let Some(tank) = &tanks[i] {
					for j in 0..tank.skeleton.node_list.len() {
						let node_index = tank.skeleton.node_list[j];
						glutil::bind_matrix4(shadow_shader, "mvp", &(shadow_projection * shadow_from_world * tank.bones[node_index].transform));

						tank.skeleton.draw_bone(j);
					}
				}
			}

			//Render shells
			gl::UseProgram(shadow_shader_instanced);
			gl::BindVertexArray(shell_mesh.vao);
			glutil::bind_matrix4(shadow_shader_instanced, "view_projection", &(shadow_projection * shadow_from_world));
			gl::DrawElementsInstanced(gl::TRIANGLES, shell_mesh.index_count, gl::UNSIGNED_SHORT, ptr::null(), shells.count() as GLint);

			//-----------Main scene rendering-----------

			//Bind first ping-pong fbo
			screen_state.ping_pong_fbos[0].bind();
			
			//Set polygon fill mode
			if is_wireframe { gl::PolygonMode(gl::FRONT_AND_BACK, gl::LINE); }
			else { gl::PolygonMode(gl::FRONT_AND_BACK, gl::FILL); }

			//Bind program for texture-mapped objects
			gl::UseProgram(mapped_shader);

			//Set uniforms that are constant for the lifetime of the program
			initialize_texture_samplers(mapped_shader, &TEXTURE_MAP_IDENTIFIERS);
			glutil::bind_matrix4(mapped_shader, "shadow_matrix", &(shadow_projection * shadow_from_world));
			glutil::bind_vector4(mapped_shader, "sun_direction", &sun_direction);
			
			gl::ActiveTexture(gl::TEXTURE3);
			gl::BindTexture(gl::TEXTURE_2D, shadow_rendertarget.texture);

			//Render static pieces of the arena
			for piece in arena_pieces.iter() {
				glutil::bind_matrix4(mapped_shader, "mvp", &(screen_state.clipping_from_world * piece.model_matrix));
				glutil::bind_matrix4(mapped_shader, "model_matrix", &piece.model_matrix);
				bind_texture_maps(&[piece.albedo, piece.normal]);

				gl::BindVertexArray(piece.vao);
				gl::DrawElements(gl::TRIANGLES, piece.index_count, gl::UNSIGNED_SHORT, ptr::null());
			}

			//Render the tanks
			gl::BindVertexArray(tank_skeleton.vao);
			for i in 0..tanks.len() {
				if let Some(tank) = &tanks[i] {
					for j in 0..tank.skeleton.node_list.len() {
						let node_index = tank.skeleton.node_list[j];
						glutil::bind_matrix4(mapped_shader, "mvp", &(screen_state.clipping_from_world * tank.bones[node_index].transform));
						glutil::bind_matrix4(mapped_shader, "model_matrix", &tank.bones[node_index].transform);
						bind_texture_maps(&[tank.skeleton.albedo_maps[j], tank.skeleton.normal_maps[j], tank.skeleton.roughness_maps[j]]);
		
						tank.skeleton.draw_bone(j);						
					}
				}
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
			glutil::bind_matrix4(mapped_instanced_shader, "view_projection", &screen_state.clipping_from_world);
			gl::DrawElementsInstanced(gl::TRIANGLES, shell_mesh.index_count, gl::UNSIGNED_SHORT, ptr::null(), shells.count() as GLint);

			//-----------Apply post-processing effects-----------
			gl::PolygonMode(gl::FRONT_AND_BACK, gl::FILL);			//Disable wireframe rendering for this section if it was enabled
			gl::BindVertexArray(postprocessing_vao);				//Bind the VAO that just defines a screen-filling triangle
			gl::ActiveTexture(gl::TEXTURE0);

			//Apply the active image effect
			match image_effect {
				//TODO: This blur is horribly inefficient. Need to use a downscaled version of the FBO (utilize mipmapping?) and fewer blur passes
				ImageEffect::Blur => {
					let passes = 8;
					initialize_texture_samplers(passthrough_shader, &["image_texture"]);
					initialize_texture_samplers(gaussian_shader, &["image_texture"]);
	
					gl::UseProgram(gaussian_shader);
					for _ in 0..passes {
						//Do a horizontal pass followed by a vertical one. This reduces complexity from N^2 to 2N
						for i in 0..screen_state.ping_pong_fbos.len() {
							screen_state.ping_pong_fbos[i ^ 1].bind();
							gl::BindTexture(gl::TEXTURE_2D, screen_state.ping_pong_fbos[i].texture);
							glutil::bind_int(gaussian_shader, "horizontal", i as GLint ^ 1);		//Flag if this is a horizontal or vertical blur pass
							gl::DrawElements(gl::TRIANGLES, 3, gl::UNSIGNED_SHORT, ptr::null());
						}
					}
	
					//Render result to the default framebuffer
					screen_state.default_framebuffer.bind();
					gl::UseProgram(passthrough_shader);
					gl::BindTexture(gl::TEXTURE_2D, screen_state.ping_pong_fbos[0].texture);
					gl::DrawElements(gl::TRIANGLES, 3, gl::UNSIGNED_SHORT, ptr::null());
				}
				ImageEffect::None => {
					screen_state.default_framebuffer.bind();
					gl::UseProgram(passthrough_shader);
					gl::BindTexture(gl::TEXTURE_2D, screen_state.ping_pong_fbos[0].texture);
					gl::DrawElements(gl::TRIANGLES, 3, gl::UNSIGNED_SHORT, ptr::null());
				}
			}

			//Before rendering 2D elements			
			gl::Disable(gl::DEPTH_TEST);			//Disable depth testing

			//Render UI buttons
			if let Some(vao) = ui_state.buttons_vao {
				draw_ui_elements(vao, ui_shader, ui_state.button_count(), &screen_state.clipping_from_screen);
			}

			//Render text
			if let Some(vao) = ui_state.glyph_vao {
				initialize_texture_samplers(glyph_shader, &["glyph_texture"]);
				gl::ActiveTexture(gl::TEXTURE0);
				gl::BindTexture(gl::TEXTURE_2D, ui_state.glyph_texture);

				draw_ui_elements(vao, glyph_shader, ui_state.glyph_count, &screen_state.clipping_from_screen);
			}
		}

		window.render_context().swap_buffers();
		glfw.poll_events();
    }
}
