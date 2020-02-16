extern crate nalgebra_glm as glm;
use std::ptr;
use glfw::{Context, WindowEvent, WindowMode};
use gl::types::*;
use ozy_engine::{glutil, init};

fn main() {
	let window_size = (1920, 1080);
	let (mut glfw, mut window, events) = init::glfw_window(window_size, WindowMode::Windowed, 3, 3, "Whee Tanks");

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

	//Compile shaders
	let mapped_shader = unsafe { glutil::compile_program_from_files("shaders/mapped_vertex.glsl", "shaders/mapped_fragment.glsl") };

	//Define the floor plane
	let (floor_vao, floor_texture, floor_matrix) = unsafe {
		let vertices = [
			//Positions				Normals						Texture coordinates
			-0.5, 0.0, -0.5,		0.0, 1.0, 0.0,				0.0, 0.0,
			0.5, 0.0, -0.5,			0.0, 1.0, 0.0,				1.0, 0.0,
			-0.5, 0.0, 0.5,			0.0, 1.0, 0.0,				0.0, 1.0,
			0.5, 0.0, 0.5,			0.0, 1.0, 0.0,				1.0, 1.0
		];
		let indices = [
			0u16, 1, 2,
			3, 2, 1
		];
		let tex_params = [
			(gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE),
			(gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE),
			(gl::TEXTURE_MIN_FILTER, gl::LINEAR),
			(gl::TEXTURE_MAG_FILTER, gl::LINEAR)
		];
		let matrix: glm::TMat4<f32> = glm::identity();

		(
			glutil::create_vertex_array_object(&vertices, &indices, &[3, 3, 2]),
			glutil::load_texture("textures/checkerboard.jpg", &tex_params),
			matrix
		)
	};
	
	//Main loop
    while !window.should_close() {
        for (_, event) in glfw::flush_messages(&events) {
            match event {
                WindowEvent::Close => { window.set_should_close(true); }
                _ => {}
            }
        }

		let view_matrix = glm::look_at(&glm::vec3(0.0, 1.0, -1.0), &glm::vec3(0.0, 0.0, 0.0), &glm::vec3(0.0, 1.0, 0.0));
		let ortho_size = 3.0;
		let projection_matrix = glm::ortho(-ortho_size, ortho_size, -ortho_size, ortho_size, -ortho_size, ortho_size);
		
		//-----------Rendering-----------
		unsafe {
			gl::Viewport(0, 0, window_size.0 as GLsizei, window_size.1 as GLsizei);
			gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);

			//Render floor plane
			gl::UseProgram(mapped_shader);
			glutil::bind_matrix4(mapped_shader, "mvp", &(projection_matrix * view_matrix * floor_matrix));
			glutil::bind_matrix4(mapped_shader, "model_matrix", &floor_matrix);
			gl::ActiveTexture(gl::TEXTURE0);
			gl::BindTexture(gl::TEXTURE_2D, floor_texture);
			gl::BindVertexArray(floor_vao);
			gl::DrawElements(gl::TRIANGLES, 6, gl::UNSIGNED_SHORT, ptr::null());
		}

		window.render_context().swap_buffers();
		glfw.poll_events();
    }
}
