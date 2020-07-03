extern crate nalgebra_glm as glm;
use std::{mem, ptr};
use std::collections::HashMap;
use std::os::raw::c_void;
use std::time::Instant;
use glfw::{Action, Context, Key, WindowEvent, WindowMode};
use gl::types::*;
use ozy_engine::{glutil, init, routines};
use crate::structs::*;

mod structs;

fn main() {
	let window_size = (1920, 1080);
	let aspect_ratio = window_size.0 as f32 / window_size.1 as f32;
	let (mut glfw, mut window, events) = init::glfw_window(window_size, WindowMode::Windowed, 3, 3, "Whee! Tanks! for ipad");

	//Make the window non-resizable
	window.set_resizable(false);

	//Configure which window events GLFW will listen for
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
		gl::CullFace(gl::BACK);											//Cull backfaces
		gl::Enable(gl::CULL_FACE);										//Enable said backface culling
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

	let mut texture_keeper = HashMap::new();

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
			0u16, 1, 2,
			3, 2, 1
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

	//Load the tank's graphics
	let mut turret_origin = glm::zero();
	let mut tank = match routines::load_ozymesh("models/tank.ozy") {
		Some(meshdata) => {
			let mut node_list = Vec::with_capacity(meshdata.names.len());
			let mut albedo_maps = Vec::with_capacity(meshdata.names.len());
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

				//Load texture
				let path = format!("textures/{}/albedo.png", meshdata.texture_names[i]);
				match texture_keeper.get(&path) {
					Some(id) => {
						albedo_maps.push(*id);
					}
					None => {
						let tex = unsafe { glutil::load_texture(&path, &default_tex_params) };
						texture_keeper.insert(path, tex);
						albedo_maps.push(tex);
					}
				}

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
				albedo_maps
			};

			//Load the tank's gameplay data
			let tank_forward = glm::vec3(-1.0, 0.0, 0.0);
			let turret_forward = tank_forward;
			let tank_position = glm::vec3(0.0, 0.0, 0.0);
			let tank_speed = 2.5;
			Tank {
				position: tank_position,
				speed: tank_speed,
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

	//The view-projection matrix is constant
	let view_matrix = glm::mat4(-1.0, 0.0, 0.0, 0.0,
								0.0, 1.0, 0.0, 0.0,
								0.0, 0.0, 1.0, 0.0,
								0.0, 0.0, 0.0, 1.0) * glm::look_at(&glm::vec3(0.0, 1.5, -1.0), &glm::vec3(0.0, 0.0, 0.0), &glm::vec3(0.0, 1.0, 0.0));
	let inverse_view_matrix = glm::affine_inverse(view_matrix);
	let ortho_size = 5.0;
	let projection_matrix = glm::ortho(-ortho_size*aspect_ratio, ortho_size*aspect_ratio, -ortho_size, ortho_size, -ortho_size, ortho_size);
	let viewprojection_matrix = projection_matrix * view_matrix;
	let inverse_viewprojection_matrix = glm::affine_inverse(viewprojection_matrix);
	let world_space_look_direction = inverse_view_matrix * glm::vec4(0.0, 0.0, 1.0, 0.0);

	//Set up the light source
	let sun_direction = glm::normalize(&glm::vec4(1.0, 1.0, 1.0, 0.0));

	let mut last_frame_instant = Instant::now();
	let mut elapsed_time = 0.0;
	let mut world_space_mouse = inverse_viewprojection_matrix * glm::vec4(0.0, 0.0, 0.0, 1.0);

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
						Key::Escape => {
							window.set_should_close(true);
						}
						Key::Q => {
							is_wireframe = !is_wireframe;
						}
						Key::W => {
							tank.move_state = TankMoving::Forwards
						}
						Key::S => {
							tank.move_state = TankMoving::Backwards
						}
						Key::A => {
							tank.tank_rotating = Rotating::Left;
						}
						Key::D => {
							tank.tank_rotating = Rotating::Right;
						}
						_ => {}
					}
				}
				WindowEvent::Key(key, _, Action::Release, ..) => {
					match key {
						Key::W | Key::S => {
							tank.move_state = TankMoving::Not;
						}
						Key::A | Key::D => {
							tank.tank_rotating = Rotating::Not;
						}
						_ => {}
					}
				}
				WindowEvent::CursorPos(x, y) => {
					let clipping_space_mouse = glm::vec4(x as f32 / (window_size.0 as f32 / 2.0) - 1.0, y as f32 / (window_size.1 as f32 / 2.0) - 1.0, 0.0, 1.0);
					world_space_mouse = inverse_viewprojection_matrix * clipping_space_mouse;
				}
                _ => {}
            }
        }
		
		//-----------Simulating-----------

		//Update the tank's position
		match tank.move_state {
			TankMoving::Forwards => {
				tank.position += tank.forward * -tank.speed * time_delta;
			}
			TankMoving::Backwards => {
				tank.position += tank.forward * tank.speed * time_delta;
			}
			TankMoving::Not => {}
		}

		//Update the tank's forward vector
		tank.forward = match tank.tank_rotating {
			Rotating::Left => {
				glm::vec4_to_vec3(&(glm::rotation(-glm::half_pi::<f32>() * time_delta, &glm::vec3(0.0, 1.0, 0.0)) * glm::vec3_to_vec4(&tank.forward)))
			}
			Rotating::Right => {
				glm::vec4_to_vec3(&(glm::rotation(glm::half_pi::<f32>() * time_delta, &glm::vec3(0.0, 1.0, 0.0)) * glm::vec3_to_vec4(&tank.forward)))
			}
			Rotating::Not => { tank.forward }
		};

		//Calculate turret rotation
		//Simple ray-plane intersection.
		let turret_rotation = {
			let plane_normal = glm::vec3(0.0, 1.0, 0.0);
			let t = glm::dot(&glm::vec4_to_vec3(&(turret_origin - world_space_mouse)), &plane_normal) / glm::dot(&glm::vec4_to_vec3(&world_space_look_direction), &plane_normal);
			let mut intersection = world_space_mouse + t * world_space_look_direction;
			intersection.z *= -1.0;
			let turret_vector = glm::normalize(&(intersection - turret_origin));
			let angle = glm::dot(&tank.forward, &glm::vec4_to_vec3(&turret_vector));
			println!("{:?}", turret_vector);
			glm::rotation(angle, &glm::vec3(0.0, 1.0, 0.0))
		};

		let tank_angle = if tank.forward.x > 0.0 {
			f32::acos(glm::dot(&tank.forward, &glm::vec3(0.0, 0.0, 1.0)))
		} else {
			-f32::acos(glm::dot(&tank.forward, &glm::vec3(0.0, 0.0, 1.0)))
		};

		tank.skeleton.node_data[0].transform = glm::translation(&tank.position) * glm::rotation(tank_angle, &glm::vec3(0.0, 1.0, 0.0));
		tank.skeleton.node_data[1].transform = turret_rotation * glm::rotation(-tank_angle, &glm::vec3(0.0, 1.0, 0.0));

		//-----------Rendering-----------
		unsafe {
			//Set the viewport
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
			gl::BindVertexArray(tank.skeleton.vao);
			for i in 0..tank.skeleton.node_list.len() {
				gl::ActiveTexture(gl::TEXTURE0);
				gl::BindTexture(gl::TEXTURE_2D, tank.skeleton.albedo_maps[i]);

				let mut model_matrix = glm::identity();
				let mut current_node = tank.skeleton.node_list[i];
				while let Some(id) = tank.skeleton.node_data[current_node].parent {
					model_matrix = tank.skeleton.node_data[current_node].transform * model_matrix;
					current_node = id;
				}
				model_matrix = tank.skeleton.node_data[current_node].transform * model_matrix;

				glutil::bind_matrix4(mapped_shader, "mvp", &(viewprojection_matrix * model_matrix));

				gl::DrawElements(gl::TRIANGLES, (tank.skeleton.geo_boundaries[i + 1] - tank.skeleton.geo_boundaries[i]) as i32, gl::UNSIGNED_SHORT, (mem::size_of::<u16>() * tank.skeleton.geo_boundaries[i] as usize) as *const c_void);
			}
		}

		window.render_context().swap_buffers();
		glfw.poll_events();
    }
}
