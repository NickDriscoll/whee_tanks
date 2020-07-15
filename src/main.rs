extern crate nalgebra_glm as glm;
use std::{mem, ptr};
use std::os::raw::c_void;
use std::time::Instant;
use glfw::{Action, Context, Key, MouseButton, WindowEvent, WindowMode};
use gl::types::*;
use ozy_engine::{glutil, init, routines};
use ozy_engine::structs::OptionVec;
use crate::structs::*;

mod structs;

const DEFAULT_TEX_PARAMS: [(GLenum, GLenum); 4] = [
	(gl::TEXTURE_WRAP_S, gl::REPEAT),
	(gl::TEXTURE_WRAP_T, gl::REPEAT),
	(gl::TEXTURE_MIN_FILTER, gl::LINEAR),
	(gl::TEXTURE_MAG_FILTER, gl::LINEAR)
];

fn main() {
	let mut window_size = (1920, 1080);
	let mut aspect_ratio = window_size.0 as f32 / window_size.1 as f32;

	let (mut glfw, mut window, events) = init::glfw_window(window_size, WindowMode::Windowed, 3, 3, "Whee! Tanks! for ipad");

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

	//Load all OpenGL function pointers
	gl::load_with(|symbol| window.get_proc_address(symbol) as *const _);

	//OpenGL static configuration
	unsafe {
		gl::Enable(gl::DEPTH_TEST);										//Enable depth testing
		gl::CullFace(gl::BACK);											//Cull backfaces
		gl::Enable(gl::CULL_FACE);										//Enable said backface culling
		gl::DepthFunc(gl::LESS);										//Pass the fragment with the smallest z-value.
		gl::Enable(gl::FRAMEBUFFER_SRGB); 								//Enable automatic linear->SRGB space conversion
		gl::Enable(gl::BLEND);											//Enable alpha blending
		gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);			//Set blend func to (Cs * alpha + Cd * (1.0 - alpha))
		gl::ClearColor(0.53, 0.81, 0.92, 1.0);							//Set clear color. A pleasant blue
	}

	//Compile shader program
	let mapped_shader = unsafe { glutil::compile_program_from_files("shaders/mapped.vert", "shaders/mapped.frag") };

	let mut texture_keeper = TextureKeeper::new();

	let mut arena_pieces = Vec::new();

	//Define the floor plane
	let arena_ratio = 16.0 / 9.0;
	unsafe {
		let tex_scale = 2.0;
		let vertices = [
			//Positions							Tangents					Bitangents				Normals							Texture coordinates
			-4.5*arena_ratio, 0.0, -5.0,		1.0, 0.0, 0.0,				0.0, 0.0, 1.0,			0.0, 1.0, 0.0,					0.0, 0.0,
			4.5*arena_ratio, 0.0, -5.0,			1.0, 0.0, 0.0,				0.0, 0.0, 1.0,			0.0, 1.0, 0.0,					tex_scale*arena_ratio, 0.0,
			-4.5*arena_ratio, 0.0, 5.0,			1.0, 0.0, 0.0,				0.0, 0.0, 1.0,			0.0, 1.0, 0.0,					0.0, tex_scale,
			4.5*arena_ratio, 0.0, 5.0,			1.0, 0.0, 0.0,				0.0, 0.0, 1.0,			0.0, 1.0, 0.0,					tex_scale*arena_ratio, tex_scale
		];
		let indices = [
			0u16, 1, 2,
			3, 2, 1
		];

		let piece = StaticGeometry {
			vao: glutil::create_vertex_array_object(&vertices, &indices, &[3, 3, 3, 3, 2]),
			albedo: glutil::load_texture("textures/bamboo_wood_semigloss/albedo.png", &DEFAULT_TEX_PARAMS),
			normal: glutil::load_texture("textures/bamboo_wood_semigloss/normal.png", &DEFAULT_TEX_PARAMS),
			model_matrix: glm::identity(),
			index_count: indices.len() as GLsizei
		};
		arena_pieces.push(piece);
	};

	//Load the tank
	let mut turret_origin = glm::zero();
	const TANK_SPEED: f32 = 2.5;
	let mut tank = match routines::load_ozymesh("models/better_tank.ozy") {
		Some(meshdata) => {
			let mut node_list = Vec::with_capacity(meshdata.names.len());
			let mut albedo_maps = Vec::with_capacity(meshdata.names.len());
			let mut normal_maps = Vec::with_capacity(meshdata.names.len());
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
				normal_maps
			};

			//Load the tank's gameplay data
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
	let shell_mesh = match routines::load_ozymesh("models/better_shell.ozy") {
		Some(meshdata) => unsafe {
			let vao = glutil::create_vertex_array_object(&meshdata.vertex_array.vertices, &meshdata.vertex_array.indices, &meshdata.vertex_array.attribute_offsets);
			let count = meshdata.geo_boundaries[1] as GLint;
			let albedo = texture_keeper.fetch_texture(&meshdata.texture_names[0], "albedo");
			let normal = texture_keeper.fetch_texture(&meshdata.texture_names[0], "normal");

			IndividualMesh {
				vao,
				albedo_map: albedo,
				normal_map: normal,
				index_count: count as GLint
			}
		}
		None => {
			panic!("Unable to load model.");
		}
	};

	let sphere_mesh = match routines::load_ozymesh("models/sphere.ozy") {
		Some(meshdata) => unsafe {
			let vao = glutil::create_vertex_array_object(&meshdata.vertex_array.vertices, &meshdata.vertex_array.indices, &meshdata.vertex_array.attribute_offsets);
			let count = meshdata.geo_boundaries[1] as GLint;
			let albedo = texture_keeper.fetch_texture(&meshdata.texture_names[0], "albedo");
			let normal = texture_keeper.fetch_texture(&meshdata.texture_names[0], "normal");

			IndividualMesh {
				vao,
				albedo_map: albedo,
				normal_map: normal,
				index_count: count
			}
		}
		None => {
			panic!("Unable to load model.");
		}
	};

	let mut shells = OptionVec::new();

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
	let sun_direction = glm::normalize(&glm::vec4(1.0, 1.0, -1.0, 0.0));

	let mut last_frame_instant = Instant::now();
	let mut elapsed_time = 0.0;
	let mut world_space_mouse = inverse_viewprojection_matrix * glm::vec4(0.0, 0.0, 0.0, 1.0);

	let mut is_wireframe = false;
	
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
				WindowEvent::Key(key, _, Action::Press, ..) => {
					match key {
						Key::Escape => {
							window.set_should_close(true);
						}
						Key::Q => unsafe {
							is_wireframe = !is_wireframe;
							if is_wireframe {
								gl::PolygonMode(gl::FRONT_AND_BACK, gl::LINE);
							} else {
								gl::PolygonMode(gl::FRONT_AND_BACK, gl::FILL);
							}
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
					//We have to flip the y coordinate because glfw thinks (0, 0) is in the top left
					let clipping_space_mouse = glm::vec4(x as f32 / (window_size.0 as f32 / 2.0) - 1.0, -(y as f32 / (window_size.1 as f32 / 2.0) - 1.0), 0.0, 1.0);
					world_space_mouse = inverse_viewprojection_matrix * clipping_space_mouse;
				}
				WindowEvent::MouseButton(MouseButton::Button1, Action::Press, ..) => { tank.firing = true; }
                _ => {}
            }
        }
		
		//-----------Simulating-----------

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
				spawn_time: elapsed_time,
				vao: shell_mesh.vao
			});
			tank.firing = false;
		}

		//Update shells
		for i in 0..shells.len() {
			if let Some(shell) = &mut shells[i] {
				shell.position += shell.velocity * delta_time;

				//Just updating the translation part of the matrix
				shell.transform[12] = shell.position.x;
				shell.transform[13] = shell.position.y;
				shell.transform[14] = shell.position.z;
			}
		}

		let sphere_matrix = glm::rotation(elapsed_time, &glm::vec3(0.0, 1.0, 0.0));

		//-----------Rendering-----------
		unsafe {
			//Set the viewport
			gl::Viewport(0, 0, window_size.0 as GLsizei, window_size.1 as GLsizei);
			gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);

			//Bind the GLSL program
			gl::UseProgram(mapped_shader);

			//Set texture sampler values
			glutil::bind_byte(mapped_shader, "albedo_map", 0);
			glutil::bind_byte(mapped_shader, "normal_map", 1);

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
				gl::ActiveTexture(gl::TEXTURE1);
				gl::BindTexture(gl::TEXTURE_2D, tank.skeleton.normal_maps[i]);

				let node_index = tank.skeleton.node_list[i];
				glutil::bind_matrix4(mapped_shader, "mvp", &(viewprojection_matrix * tank.skeleton.node_data[node_index].transform));
				glutil::bind_matrix4(mapped_shader, "model_matrix", &tank.skeleton.node_data[node_index].transform);

				gl::DrawElements(gl::TRIANGLES, (tank.skeleton.geo_boundaries[i + 1] - tank.skeleton.geo_boundaries[i]) as i32, gl::UNSIGNED_SHORT, (mem::size_of::<u16>() * tank.skeleton.geo_boundaries[i] as usize) as *const c_void);
			}

			//Render sphere
			gl::BindVertexArray(sphere_mesh.vao);
			gl::BindTexture(gl::TEXTURE_2D, sphere_mesh.albedo_map);
			gl::ActiveTexture(gl::TEXTURE1);
			gl::BindTexture(gl::TEXTURE_2D, sphere_mesh.normal_map);
			glutil::bind_matrix4(mapped_shader, "mvp", &(viewprojection_matrix * sphere_matrix));
			glutil::bind_matrix4(mapped_shader, "model_matrix", &sphere_matrix);
			gl::DrawElements(gl::TRIANGLES, sphere_mesh.index_count, gl::UNSIGNED_SHORT, ptr::null());

			//Render tank shells
			for opt_shell in shells.iter() {
				if let Some(shell) = opt_shell {
					gl::BindVertexArray(shell_mesh.vao);
					
					gl::ActiveTexture(gl::TEXTURE0);
					gl::BindTexture(gl::TEXTURE_2D, shell_mesh.albedo_map);
					gl::ActiveTexture(gl::TEXTURE1);
					gl::BindTexture(gl::TEXTURE_2D, shell_mesh.normal_map);

					glutil::bind_matrix4(mapped_shader, "mvp", &(viewprojection_matrix * shell.transform));
					glutil::bind_matrix4(mapped_shader, "model_matrix", &shell.transform);

					gl::DrawElements(gl::TRIANGLES, shell_mesh.index_count as GLsizei, gl::UNSIGNED_SHORT, ptr::null());
				}
			}
		}

		window.render_context().swap_buffers();
		glfw.poll_events();
    }
}
