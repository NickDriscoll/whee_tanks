extern crate nalgebra_glm as glm;
use std::{mem, ptr};
use std::os::raw::c_void;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use glfw::{Action, Context, Key, WindowEvent, WindowMode};
use gl::types::*;
use ozy_engine::{glutil, init, routines};
use crate::structs::*;

mod structs;

fn main() {
	let window_size = (1920, 1080);
	let aspect_ratio = window_size.0 as f32 / window_size.1 as f32;
	let (mut glfw, mut window, events) = init::glfw_window(window_size, WindowMode::Windowed, 3, 3, "Whee! Tanks!");

	//Make the window non-resizable
	window.set_resizable(false);

	//Configure what kinds of events GLFW will listen for
	window.set_key_polling(true);
	window.set_framebuffer_size_polling(true);
	window.set_mouse_button_polling(true);
	window.set_scroll_polling(true);
	window.set_cursor_pos_polling(true);

	//Load all OpenGL function pointers
	gl::load_with(|symbol| window.get_proc_address(symbol) as *const _);

	//OpenGL configuration
	unsafe {
		gl::Enable(gl::DEPTH_TEST);										//Enable depth testing
		gl::CullFace(gl::BACK);
		gl::Enable(gl::CULL_FACE);
		gl::DepthFunc(gl::LEQUAL);										//Pass the fragment with the smallest z-value. Needs to be <= instead of < because for all skybox pixels z = 1.0
		gl::Enable(gl::FRAMEBUFFER_SRGB); 								//Enable automatic linear->SRGB space conversion
		gl::Enable(gl::BLEND);											//Enable alpha blending
		gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);			//Set blend func to (Cs * alpha + Cd * (1.0 - alpha))
		gl::ClearColor(0.53, 0.81, 0.92, 1.0);							//Set clear color. A pleasant blue
	}

	//Compile shader
	let mapped_shader = unsafe { glutil::compile_program_from_files("shaders/mapped_vertex.glsl", "shaders/mapped_fragment.glsl") };
	let default_tex_params = [
		(gl::TEXTURE_WRAP_S, gl::REPEAT),
		(gl::TEXTURE_WRAP_T, gl::REPEAT),
		(gl::TEXTURE_MIN_FILTER, gl::LINEAR),
		(gl::TEXTURE_MAG_FILTER, gl::LINEAR)
	];

	let steel_plate_albedo = unsafe { glutil::load_texture("textures/steel_plate/albedo.png", &default_tex_params) };
	let wood_veneer_albedo = unsafe { glutil::load_texture("textures/wood_veneer/albedo.png", &default_tex_params) };
	let hex_stones_albedo = unsafe { glutil::load_texture("textures/hex-stones1-bl/hex-stones1-albedo.png", &default_tex_params) };

	let mut arena_pieces = Vec::new();

	//Define the floor plane
	let arena_ratio = 16.0 / 9.0;
	unsafe {
		let tex_scale = 2.0;
		let vertices = [
			//Positions							Texture coordinates
			-4.5*arena_ratio, 0.0, -5.0,		0.0, 0.0,
			4.5*arena_ratio, 0.0, -5.0,			tex_scale*arena_ratio, 0.0,
			-4.5*arena_ratio, 0.0, 5.0,			0.0, tex_scale,
			4.5*arena_ratio, 0.0, 5.0,			tex_scale*arena_ratio, tex_scale
		];
		let indices = [
			0u16, 2, 1,
			3, 1, 2
		];

		let piece = StaticGeometry {
			vao: glutil::create_vertex_array_object(&vertices, &indices, &[3, 2]),
			albedo: glutil::load_texture("textures/bamboo_wood_semigloss/albedo.png", &default_tex_params),
			normal: glutil::load_texture("textures/bamboo_wood_semigloss/normal.png", &default_tex_params),
			model_matrix: glm::identity(),
			index_count: indices.len() as GLsizei
		};
		arena_pieces.push(piece);
	};
	
	//Set up the tanks' skeleton
	let mut tank_skeleton_nodes = vec![
		SkeletonNode {
			transform: glm::identity(),
			parent: None
		},
		SkeletonNode {
			transform: glm::identity(),
			parent: Some(0)
		}
	];
	
	let mut rendered_tank_piece = 0;

	//Load the tank model
	let tank_skeleton = match routines::load_ozymesh("models/tank.ozy") {
		Some(meshdata) => {
			let steel_plates = ["Turret", "Barrel"];
			let wood_names = ["Hull"];
			let mut nodes = Vec::with_capacity(meshdata.names.len());
			let mut albedo_maps = Vec::with_capacity(meshdata.names.len());
			for i in 0..meshdata.names.len() {
				if steel_plates.contains(&(meshdata.names[i].as_str())) {
					nodes.push(1);
					albedo_maps.push(steel_plate_albedo);
				} else if wood_names.contains(&(meshdata.names[i].as_str())) {
					nodes.push(0);
					albedo_maps.push(wood_veneer_albedo);
				} else {
					nodes.push(0);
					albedo_maps.push(hex_stones_albedo);
				}
			}

			let vao = unsafe { glutil::create_vertex_array_object(&meshdata.vertex_array.vertices, &meshdata.vertex_array.indices, &meshdata.vertex_array.attribute_offsets) };
			Skeleton {
				vao,
				nodes,
				geo_boundaries: meshdata.geo_boundaries,
				albedo_maps
			}
		}
		None => {
			panic!("Unable to load model.");
		}
	};

	//The view-projection matrix is constant
	let view_matrix = glm::look_at(&glm::vec3(0.0, 1.5, -1.0), &glm::vec3(0.0, 0.0, 0.0), &glm::vec3(0.0, 1.0, 0.0));
	let ortho_size = 5.0;
	let projection_matrix = glm::ortho(-ortho_size*aspect_ratio, ortho_size*aspect_ratio, -ortho_size, ortho_size, -ortho_size, ortho_size);

	//Set up the light source
	let sun_direction = glm::normalize(&glm::vec4(1.0, 1.0, 1.0, 0.0));

	let mut last_frame_instant = Instant::now();
	let mut elapsed_time = 0.0;

	let mut is_wireframe = false;
	
	//Main loop
    while !window.should_close() {
		//Calculate time since the last frame started in seconds
		let time_delta = {
			let frame_instant = Instant::now();
			let dur = frame_instant.duration_since(last_frame_instant);
			last_frame_instant = frame_instant;

			//There's an underlying assumption here that frames will always take less than one second to complete
			(dur.subsec_millis() as f32 / 1000.0) + (dur.subsec_micros() as f32 / 1_000_000.0)
		};
		elapsed_time += time_delta;

		//Handle window events
        for (_, event) in glfw::flush_messages(&events) {
            match event {
				WindowEvent::Close => { window.set_should_close(true); }
				WindowEvent::Key(key, _, Action::Press, ..) => {
					match key {
						Key::Q => {
							is_wireframe = !is_wireframe;
						}
						Key::Space => {
							rendered_tank_piece = (rendered_tank_piece + 1) % 5;
						}
						_ => {}
					}
				}
				WindowEvent::CursorPos(x, y) => {
					//println!("{}, {}", x, y);
				}
                _ => {}
            }
        }
		
		//-----------Simulating-----------
		tank_skeleton_nodes[0].transform = glm::translation(&glm::vec3(0.0, 1.0 + 0.05*elapsed_time, 0.0))
										 * glm::rotation(elapsed_time, &glm::vec3(0.0, 1.0, 0.0));

		//-----------Rendering-----------
		unsafe {
			//Set polygon mode
			if is_wireframe {
				gl::PolygonMode(gl::FRONT_AND_BACK, gl::LINE);
			} else {
				gl::PolygonMode(gl::FRONT_AND_BACK, gl::FILL);
			}

			gl::Viewport(0, 0, window_size.0 as GLsizei, window_size.1 as GLsizei);
			gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);

			//Bind the GLSL program
			gl::UseProgram(mapped_shader);

			//Bind the sun direction
			glutil::bind_vector4(mapped_shader, "sun_direction", &sun_direction);

			//Render static pieces of the arena
			for piece in arena_pieces.iter() {
				glutil::bind_matrix4(mapped_shader, "mvp", &(projection_matrix * view_matrix * piece.model_matrix));
				glutil::bind_matrix4(mapped_shader, "model_matrix", &piece.model_matrix);

				//Albedo map
				gl::ActiveTexture(gl::TEXTURE0);
				gl::BindTexture(gl::TEXTURE_2D, piece.albedo);

				//Normal map
				gl::ActiveTexture(gl::TEXTURE1);
				gl::BindTexture(gl::TEXTURE_2D, piece.normal);

				gl::BindVertexArray(piece.vao);
				gl::DrawElements(gl::TRIANGLES, piece.index_count, gl::UNSIGNED_SHORT, ptr::null());
			}

			//Render the tank
			gl::BindVertexArray(tank_skeleton.vao);
			for i in 0..tank_skeleton.nodes.len() {
				gl::ActiveTexture(gl::TEXTURE0);
				gl::BindTexture(gl::TEXTURE_2D, tank_skeleton.albedo_maps[i]);

				glutil::bind_matrix4(mapped_shader, "mvp", &(projection_matrix * view_matrix * tank_skeleton_nodes[0].transform));

				gl::DrawElements(gl::TRIANGLES, tank_skeleton.geo_boundaries[i + 1] - tank_skeleton.geo_boundaries[i], gl::UNSIGNED_SHORT, (mem::size_of::<u16>() as i32 * tank_skeleton.geo_boundaries[i]) as *const c_void);
			}
		}

		window.render_context().swap_buffers();
		glfw.poll_events();
    }
}
