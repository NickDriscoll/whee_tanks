extern crate nalgebra_glm as glm;
use std::ptr;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use glfw::{Context, WindowEvent, WindowMode};
use gl::types::*;
use ozy_engine::{glutil, init, routines};

fn main() {
	let window_size = (1920, 1080);
	let aspect_ratio = window_size.0 as f32 / window_size.1 as f32;
	let (mut glfw, mut window, events) = init::glfw_window(window_size, WindowMode::Windowed, 3, 3, "Whee Tanks");

	//Make the window non-resizable
	window.set_resizable(false);

	//Configure what kinds of events GLFW will listen for
	window.set_key_polling(true);
	window.set_framebuffer_size_polling(true);
	window.set_mouse_button_polling(true);
	window.set_scroll_polling(true);

	//Load all OpenGL function pointers, GLFW does all the work here
	gl::load_with(|symbol| window.get_proc_address(symbol) as *const _);

	//These OpenGL settings are only set once, so we just do it here
	unsafe {
		gl::Enable(gl::DEPTH_TEST);										//Enable depth testing
		gl::DepthFunc(gl::LEQUAL);										//Pass the fragment with the smallest z-value. Needs to be <= instead of < because for all skybox pixels z = 1.0
		gl::Enable(gl::FRAMEBUFFER_SRGB); 								//Enable automatic linear->SRGB space conversion
		gl::Enable(gl::BLEND);											//Enable alpha blending
		gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);			//Set blend func to (Cs * alpha + Cd * (1.0 - alpha))
		gl::ClearColor(0.53, 0.81, 0.92, 1.0);							//Set clear color. A pleasant blue
	}

	//Compile shader
	let mapped_shader = unsafe { glutil::compile_program_from_files("shaders/mapped_vertex.glsl", "shaders/mapped_fragment.glsl") };

	//Define the floor plane
	let arena_ratio = 16.0 / 9.0;
	let (floor_vao, floor_texture, floor_matrix, floor_count) = unsafe {
		let tex_scale = 2.0;
		let vertices = [
			//Positions				Texture coordinates
			-0.5, 0.0, -0.5,		0.0, 0.0,
			0.5, 0.0, -0.5,			tex_scale, 0.0,
			-0.5, 0.0, 0.5,			0.0, tex_scale,
			0.5, 0.0, 0.5,			tex_scale, tex_scale
		];
		let indices = [
			0u16, 1, 2,
			3, 2, 1
		];
		let tex_params = [
			(gl::TEXTURE_WRAP_S, gl::REPEAT),
			(gl::TEXTURE_WRAP_T, gl::REPEAT),
			(gl::TEXTURE_MIN_FILTER, gl::LINEAR),
			(gl::TEXTURE_MAG_FILTER, gl::LINEAR)
		];
		let matrix = glm::scaling(&glm::vec3(9.0*arena_ratio, 10.0, 10.0));

		(
			glutil::create_vertex_array_object(&vertices, &indices, &[3, 2]),
			glutil::load_texture("textures/wood_veneer/albedo.png", &tex_params),
			matrix,
			indices.len() as GLsizei
		)
	};

	//Define upper wall
	let (wall_vao, wall_texture, mut wall_matrix, wall_count) = unsafe {
		let vertices = [
			//Positions				Texture Coordinates
			-0.5, -0.5, -0.5,		0.0, 0.0,
			0.5, -0.5, -0.5,		1.0, 0.0,
			-0.5, 0.5, -0.5,		0.0, -1.0,
			0.5, 0.5, -0.5,			2.0, 0.0,
			-0.5, -0.5, 0.5,		0.0, 1.0,
			-0.5, 0.5, 0.5,			0.0, 2.0,
			0.5, -0.5, 0.5,			1.0, 1.0,
			0.5, 0.5, 0.5,			2.0, 1.0
		];
		let indices = [
			//Front
			0u16, 1, 2,
			3, 2, 1,
			
			//Left
			0, 2, 4,
			2, 5, 4,

			//Right
			3, 1, 6,
			7, 3, 6,

			//Back
			5, 7, 4,
			7, 6, 4,

			//Bottom
			4, 1, 0,
			4, 6, 1,
			
			//Top
			7, 5, 2,
			7, 2, 3
		];
		let vao = glutil::create_vertex_array_object(&vertices, &indices, &[3, 2]);

		let tex_params = [
			(gl::TEXTURE_WRAP_S, gl::REPEAT),
			(gl::TEXTURE_WRAP_T, gl::REPEAT),
			(gl::TEXTURE_MIN_FILTER, gl::LINEAR),
			(gl::TEXTURE_MAG_FILTER, gl::LINEAR)
		];
		let albedo = glutil::load_texture("textures/steel_plate/albedo.png", &tex_params);
		let matrix = glm::translation(&glm::vec3(0.0, 0.5, 5.5)) * glm::scaling(&glm::vec3(9.0*arena_ratio, 1.0, 1.0));
		(vao, albedo, matrix, indices.len() as GLsizei)
	};

	//The view-projection matrix is constant
	let view_matrix = glm::look_at(&glm::vec3(0.0, 1.5, -1.0), &glm::vec3(0.0, 0.0, 0.0), &glm::vec3(0.0, 1.0, 0.0));
	let ortho_size = 5.0;
	let projection_matrix = glm::ortho(-ortho_size*aspect_ratio, ortho_size*aspect_ratio, -ortho_size, ortho_size, -ortho_size, ortho_size);

	let mut last_frame_instant = Instant::now();
	let mut elapsed_time = 0.0;
	
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
                _ => {}
            }
        }
		
		//-----------Simulating-----------
		//wall_matrix = glm::translation(&glm::vec3(f32::sin(elapsed_time*2.0)*4.0, 1.0, f32::sin(elapsed_time))) * routines::uniform_scale(0.5);

		//-----------Rendering-----------
		unsafe {
			gl::Viewport(0, 0, window_size.0 as GLsizei, window_size.1 as GLsizei);
			gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);

			//Bind the GLSL program
			gl::UseProgram(mapped_shader);

			//Render floor plane
			glutil::bind_matrix4(mapped_shader, "mvp", &(projection_matrix * view_matrix * floor_matrix));
			glutil::bind_matrix4(mapped_shader, "model_matrix", &floor_matrix);
			gl::ActiveTexture(gl::TEXTURE0);
			gl::BindTexture(gl::TEXTURE_2D, floor_texture);
			gl::BindVertexArray(floor_vao);
			gl::DrawElements(gl::TRIANGLES, floor_count, gl::UNSIGNED_SHORT, ptr::null());

			//Render upper wall
			glutil::bind_matrix4(mapped_shader, "mvp", &(projection_matrix * view_matrix * wall_matrix));
			glutil::bind_matrix4(mapped_shader, "model_matrix", &wall_matrix);
			gl::ActiveTexture(gl::TEXTURE0);
			gl::BindTexture(gl::TEXTURE_2D, wall_texture);
			gl::BindVertexArray(wall_vao);
			gl::DrawElements(gl::TRIANGLES, wall_count, gl::UNSIGNED_SHORT, ptr::null());
		}

		window.render_context().swap_buffers();
		glfw.poll_events();
    }
}
