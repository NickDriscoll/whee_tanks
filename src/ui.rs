use crate::input::{Command};
use ozy_engine::structs::{OptionVec};
use ozy_engine::glutil;
use glyph_brush::{BrushAction, BrushError, GlyphBrush, GlyphCruncher, GlyphVertex, ab_glyph::PxScale, Section, Rectangle, Text};
use gl::types::*;
use std::os::raw::c_void;
use std::{mem, ptr};

const FLOATS_PER_GLYPH: usize = 32;
type GlyphBrushVertexType = [f32; FLOATS_PER_GLYPH];

fn insert_index_buffer_quad(index_buffer: &mut [u16], i: usize) {
	index_buffer[i * 6] = 4 * i as u16;
	index_buffer[i * 6 + 1] = index_buffer[i * 6] + 1;
	index_buffer[i * 6 + 2] = index_buffer[i * 6] + 2;
	index_buffer[i * 6 + 3] = index_buffer[i * 6] + 3;
	index_buffer[i * 6 + 4] = index_buffer[i * 6] + 2;
	index_buffer[i * 6 + 5] = index_buffer[i * 6] + 1;
}

//First argument to glyph_brush.process_queued()
unsafe fn upload_glyph_texture(glyph_texture: GLuint, rect: Rectangle<u32>, data: &[u8]) {
	gl::TextureSubImage2D(
		glyph_texture,
		0,
		rect.min[0] as _,
		rect.min[1] as _,
		rect.width() as _,
		rect.height() as _,
		gl::RED,
		gl::UNSIGNED_BYTE,
		data.as_ptr() as _
	);
}

//Second argument to glyph_brush.process_queued()
fn glyph_vertex_transform(vertex: GlyphVertex) -> GlyphBrushVertexType {
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
		left, bottom, texleft, texbottom, vertex.extra.color[0], vertex.extra.color[1], vertex.extra.color[2], vertex.extra.color[3],
		right, bottom, texright, texbottom, vertex.extra.color[0], vertex.extra.color[1], vertex.extra.color[2], vertex.extra.color[3],
		left, top, texleft, textop, vertex.extra.color[0], vertex.extra.color[1], vertex.extra.color[2], vertex.extra.color[3],
		right, top, texright, textop, vertex.extra.color[0], vertex.extra.color[1], vertex.extra.color[2], vertex.extra.color[3]
	]	
}

//Subset of UIState created to fix some borrowing issues
pub struct UIInternals<'a> {
    vao_flag: bool,
    pub glyph_brush: &'a mut GlyphBrush<GlyphBrushVertexType>,
    buttons: OptionVec<UIButton>,
    sections: OptionVec<Section<'a>>
}

pub struct UIState<'a> {
    pub button_color_buffer: GLuint,
    pub buttons_vao: Option<GLuint>,
    pub internals: UIInternals<'a>,
    pub glyph_texture: GLuint,
    pub glyph_vao: Option<GLuint>,
	pub glyph_count: usize,
	menu_chains: Vec<Vec<usize>>, //Array of array of menu ids used for nested menu traversal
    menus: Vec<Menu<'a>>
}

impl<'a> UIState<'a> {
    pub const FLOATS_PER_COLOR: usize = 4;
    pub const COLORS_PER_BUTTON: usize = 4;

    pub fn new(glyph_brush: &'a mut GlyphBrush<GlyphBrushVertexType>) -> Self {
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

        UIState {
            button_color_buffer: 0,
            buttons_vao: None,
            internals: UIInternals::new(glyph_brush),
            glyph_texture,
            glyph_vao: None,
			glyph_count: 0,
			menu_chains: Vec::new(),
            menus: Vec::new()
        }
    }
    
    pub fn add_section(&mut self, section: Section<'a>) -> usize { self.internals.add_section(section) } //Adds a standalone section to the UI
	
	pub fn append_to_chain(&mut self, chain: usize, dst: usize) {
		//We only need to hide the current menu if there are more than zero menus in the chain
		if self.menu_chains[chain].len() > 0 {
			let src = self.menu_chains[chain][self.menu_chains[chain].len() - 1];
			self.hide_menu(src);
		}
		self.show_menu(dst);
		self.menu_chains[chain].push(dst);
	}

    pub fn button_count(&self) -> usize { self.internals.buttons.count() }

	pub fn create_menu_chain(&mut self) -> usize {
		let mut chain = Vec::new();
		self.menu_chains.push(chain);
		self.menu_chains.len() - 1
	}

	pub fn delete_section(&mut self, index: usize) { self.internals.delete_section(index); }

	pub fn display_screen(&mut self, section: Section<'a>, menu: usize, chain: usize) -> usize {		
		self.append_to_chain(chain, menu);
		self.add_section(section)
	}

    pub fn hide_all_menus(&mut self) {
        for menu in self.menus.iter_mut() {
            menu.hide(&mut self.internals);
        }
    }

	fn hide_menu(&mut self, index: usize) { self.menus[index].hide(&mut self.internals); }

	pub fn hide_screen(&mut self, section_index: usize, chain: usize) {
		self.rollback_chain(chain);
		self.delete_section(section_index);
	}

    //Clears the data in self.internals and marks all menus as inactive
    pub fn reset(&mut self) {
        self.internals.buttons.clear();
        self.internals.sections.clear();
        for menu in self.menus.iter_mut() {
			menu.active = false;
		}
		
		for chain in self.menu_chains.iter_mut() {
			chain.clear();
		}
    }
	
	pub fn rollback_chain(&mut self, chain: usize) {
		if let Some(index) = self.menu_chains[chain].pop() {
			self.hide_menu(index);
			
			if self.menu_chains[chain].len() > 0 {
				let dst = self.menu_chains[chain][self.menu_chains[chain].len() - 1];
				self.show_menu(dst);
			}
		}
	}

	pub fn set_menus(&mut self, menus: Vec<Menu<'a>>) {
		self.menus = menus;
	}

	fn show_menu(&mut self, index: usize) { self.menus[index].show(&mut self.internals); }

	//Call this function each frame right before rendering
    pub fn synchronize(&mut self) {
		//Queue glyph_brush sections
		self.queue_sections();

		//glyph_brush processing
		self.glyph_processing();

		//Create vao for the ui buttons
		self.update_button_vao();
    }

    pub fn toggle_menu(&mut self, chain: usize, menu: usize) {
		if self.menus[menu].active {
			self.rollback_chain(chain);
		} else {
			self.append_to_chain(chain, menu);
		}
	}

    //Gets input from the UI system and generates Commands for the command buffer I.E. user clicking on buttons
    //Also updates the instanced color buffer used for rendering the buttons
    //Meant to be called once per frame
    pub fn update_buttons(&mut self, screen_space_mouse: glm::TVec2<f32>, mouse_lbutton_pressed: bool, mouse_lbutton_pressed_last_frame: bool, command_buffer: &mut Vec<Command>) {        
		//Handle input from the UI buttons
		let mut current_button = 0;
		for i in 0..self.internals.buttons.len() {
			if let Some(button) = self.internals.buttons.get_mut_element(i) {
				if screen_space_mouse.x > button.bounds.min[0] &&
				   screen_space_mouse.x < button.bounds.max[0] &&
				   screen_space_mouse.y > button.bounds.min[1] &&
				   screen_space_mouse.y < button.bounds.max[1] {

					if mouse_lbutton_pressed_last_frame && !mouse_lbutton_pressed {
						if let Some(command) = button.command {
							command_buffer.push(command);
						}
					}

					//Handle updating button graphics
					if button.state == ButtonState::None || (mouse_lbutton_pressed == mouse_lbutton_pressed_last_frame) {
						let color = if mouse_lbutton_pressed {
							[0.0, 0.8, 0.0, 0.5]
						} else {
							[0.0, 0.4, 0.0, 0.5]
						};
						unsafe { Self::update_ui_button_color(self.button_color_buffer, current_button, color); }

						button.state = ButtonState::Highlighted;
					}
				} else {
					if button.state != ButtonState::None {
						let color = [0.0, 0.0, 0.0, 0.5];
						unsafe { Self::update_ui_button_color(self.button_color_buffer, current_button, color); }

						button.state = ButtonState::None;
					}
				}				
				current_button += 1;
			}
		}
	}
	
	pub fn update_screen_size(&mut self, size: (u32, u32)) {
		for menu in self.menus.iter_mut() {
			match menu.anchor {
				UIAnchor::DeadCenter(_) => {
					let ugh = (size.0 as f32, size.1 as f32);
					menu.anchor = UIAnchor::DeadCenter(ugh);
					if menu.active {
						menu.toggle(&mut self.internals);
						menu.toggle(&mut self.internals);
					}
				}
				_ => {}
			}
		}
	}

    fn glyph_processing(&mut self) {
        let glyph_tex = self.glyph_texture;

        //glyph_brush processing
		let mut glyph_result = self.internals.glyph_brush.process_queued(|rect, tex_data| unsafe { 
			upload_glyph_texture(glyph_tex, rect, tex_data);
		}, glyph_vertex_transform);

		//Repeatedly resize the glyph texture until the error stops
		while let Err(BrushError::TextureTooSmall { suggested }) = glyph_result {
			let (width, height) = suggested;
			unsafe {
				gl::BindTexture(gl::TEXTURE_2D, self.glyph_texture);
				gl::TexImage2D(gl::TEXTURE_2D, 0, gl::RED as GLint, width as GLint, height as GLint, 0, gl::RED, gl::UNSIGNED_BYTE, ptr::null());
			}
			self.internals.glyph_brush.resize_texture(width, height);
			glyph_result = self.internals.glyph_brush.process_queued(|rect, tex_data| unsafe {
				upload_glyph_texture(glyph_tex, rect, tex_data);
			}, glyph_vertex_transform);
		}
		
		//This should never fail
		match glyph_result.unwrap() {
			BrushAction::Draw(verts) => {
				if verts.len() > 0 {
					let mut vertex_buffer = Vec::with_capacity(verts.len() * FLOATS_PER_GLYPH);
					let mut index_buffer = vec![0; verts.len() * 6];
					for i in 0..verts.len() {
						for v in verts[i].iter() {
							vertex_buffer.push(*v);
						}
						
						//Fill out index buffer
						insert_index_buffer_quad(&mut index_buffer, i);
					}
					self.glyph_count = verts.len();

					let attribute_strides = [2, 2, 4];
					match self.glyph_vao {
						Some(mut vao) => unsafe {
							gl::DeleteVertexArrays(1, &mut vao);
							self.glyph_vao = Some(glutil::create_vertex_array_object(&vertex_buffer, &index_buffer, &attribute_strides));
						}
						None => unsafe {
							self.glyph_vao = Some(glutil::create_vertex_array_object(&vertex_buffer, &index_buffer, &attribute_strides));
						}
					}
				} else {
					if let Some(mut vao) = self.glyph_vao {
						unsafe { gl::DeleteVertexArrays(1, &mut vao); }
						self.glyph_vao = None;
					}
				}
			}
			BrushAction::ReDraw => {}
		}
    }

    fn queue_sections(&mut self) {
        for sec in self.internals.sections.iter() {
			if let Some(s) = sec {
				self.internals.glyph_brush.queue(s);
			}
		}
    }

    fn update_button_vao(&mut self) {
        //Create vao for the ui buttons
		if self.internals.vao_flag && self.button_count() > 0 {
			self.internals.vao_flag = false;
			unsafe { 
				let floats_per_button = 4 * 2;
				let mut vertices = vec![0.0; self.button_count() * floats_per_button];
				let mut indices = vec![0u16; self.button_count() * 6];

				let mut quads_added = 0;
				for i in 0..self.internals.buttons.len() {
					if let Some(button) = &self.internals.buttons[i] {
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

				match self.buttons_vao {
					Some(mut vao) => {
						gl::DeleteVertexArrays(1, &mut vao);
						self.buttons_vao = Some(glutil::create_vertex_array_object(&vertices, &indices, &[2]));
						gl::BindVertexArray(vao);
					}
					None => {
						let vao = glutil::create_vertex_array_object(&vertices, &indices, &[2]);
						self.buttons_vao = Some(vao);
						gl::BindVertexArray(vao);
					}
				}

				//Create GPU buffer for ui button colors
				self.button_color_buffer = {
					let element_count = self.button_count() * UIState::COLORS_PER_BUTTON * UIState::FLOATS_PER_COLOR;

					let mut data = vec![0.0f32; element_count];
					for i in 0..(data.len() / UIState::FLOATS_PER_COLOR) {
						data[i * 4] = 0.0;
						data[i * 4 + 1] = 0.0;
						data[i * 4 + 2] = 0.0;
						data[i * 4 + 3] = 0.5;
					}

					let mut b = 0;
					gl::GenBuffers(1, &mut b);
					gl::BindBuffer(gl::ARRAY_BUFFER, b);
					gl::BufferData(gl::ARRAY_BUFFER, (element_count * mem::size_of::<GLfloat>()) as GLsizeiptr, &data[0] as *const f32 as *const c_void, gl::DYNAMIC_DRAW);

					//Attach buffer to vao
					gl::VertexAttribPointer(1,
											4,
											gl::FLOAT,
											gl::FALSE,
											(UIState::FLOATS_PER_COLOR * mem::size_of::<GLfloat>()) as GLsizei,
											ptr::null());
					gl::EnableVertexAttribArray(1);

					b
				};
			}
		} else if self.button_count() == 0 {
			if let Some(mut vao) = self.buttons_vao {
				unsafe { gl::DeleteVertexArrays(1, &mut vao); }
				self.buttons_vao = None;
			}
		}
    }

    //Change the color of button at index to color
    unsafe fn update_ui_button_color(buffer: GLuint, index: usize, color: [f32; 4]) { //When color's size is Self::FLOATS_PER_COLOR it causes a compiler bug
        let mut data = vec![0.0; Self::FLOATS_PER_COLOR * Self::COLORS_PER_BUTTON];
        
        for i in 0..(data.len() / Self::FLOATS_PER_COLOR) {
            data[i * Self::FLOATS_PER_COLOR] = color[0];
            data[i * Self::FLOATS_PER_COLOR + 1] = color[1];
            data[i * Self::FLOATS_PER_COLOR + 2] = color[2];
            data[i * Self::FLOATS_PER_COLOR + 3] = color[3];
        }
        gl::BindBuffer(gl::ARRAY_BUFFER, buffer);
        gl::BufferSubData(gl::ARRAY_BUFFER,
                        (Self::COLORS_PER_BUTTON * Self::FLOATS_PER_COLOR * index * mem::size_of::<GLfloat>()) as GLintptr,
                        (Self::FLOATS_PER_COLOR * Self::COLORS_PER_BUTTON * mem::size_of::<GLfloat>()) as GLsizeiptr,
                        &data[0] as *const GLfloat as *const c_void);
    }
}

impl<'a> UIInternals<'a> {
    pub fn new(glyph_brush: &'a mut GlyphBrush<GlyphBrushVertexType>) -> Self {
        UIInternals {
            vao_flag: false,
            glyph_brush,
            buttons: OptionVec::new(),
            sections: OptionVec::new()
        }
    }

    pub fn add_button(&mut self, button: UIButton) -> usize {
        self.vao_flag = true;
        self.buttons.insert(button)
    }

    pub fn add_section(&mut self, section: Section<'a>) -> usize {
        self.vao_flag = true;
        self.sections.insert(section)
    }

    pub fn delete_button(&mut self, index: usize) {
        self.vao_flag = true;
        if let Some(button) = &self.buttons[index] {
            self.sections.delete(button.section_id());
            self.buttons.delete(index);
        }
    }

    pub fn delete_section(&mut self, index: usize) {
        self.vao_flag = true;
        self.sections.delete(index);
    }
}

#[derive(Debug)]
pub struct UIButton {
    pub bounds: glyph_brush::Rectangle<f32>,
    pub state: ButtonState,
    pub command: Option<Command>,
    section_id: usize
}

impl UIButton {
    pub fn new(section_id: usize, bounds: glyph_brush::Rectangle<f32>, command: Option<Command>) -> Self {
        UIButton {
            bounds,
            state: ButtonState::None,
            command,
            section_id
        }
    }

    pub fn section_id(&self) -> usize { self.section_id }
}

pub struct Menu<'a> {
	button_labels: Vec<&'a str>,
	button_commands: Vec<Option<Command>>,
	label_colors: Vec<[f32; 4]>,
    anchor: UIAnchor,
    active: bool,
    ids: Vec<usize> //Indices into the buttons OptionVec. These are only valid when self.active == true
}

impl<'a> Menu<'a> {
    pub fn new(buttons: Vec<(&'a str, Option<Command>)>, anchor: UIAnchor) -> Self {
		let size = buttons.len();
		let mut button_labels = Vec::with_capacity(size);
		let mut button_commands = Vec::with_capacity(size);
		for butt in buttons.iter() {
			button_labels.push(butt.0);
			button_commands.push(butt.1);
		}

		let label_colors = vec![[1.0, 1.0, 1.0, 1.0]; size];
		
        Menu {
            button_labels,
            button_commands,
            label_colors,
            anchor,
            active: false,
            ids: vec![0; size]
        }
	}
	
	pub fn new_with_colors(buttons: Vec<(&'a str, Option<Command>, [f32; 4])>, anchor: UIAnchor) -> Self {
		let size = buttons.len();
		let mut button_labels = Vec::with_capacity(size);
		let mut button_commands = Vec::with_capacity(size);
		let mut label_colors = Vec::with_capacity(size);
		for butt in buttons.iter() {
			button_labels.push(butt.0);
			button_commands.push(butt.1);
			label_colors.push(butt.2);
		}

        Menu {
            button_labels,
            button_commands,
            label_colors,
            anchor,
            active: false,
            ids: vec![0; size]
        }
	}

    //Adds this menu's data to the arrays of buttons and sections
    pub fn show(&mut self, ui_internals: &mut UIInternals<'a>) {
        if self.active { return; }

        //Submit the pause menu data
		const BORDER_WIDTH: f32 = 15.0;
		const BUFFER_DISTANCE: f32 = 10.0;
		let font_size = 36.0;
		for i in 0..self.button_labels.len() {
			let mut section = {
				let section = Section::new();
				let mut text = Text::new(self.button_labels[i]).with_color(self.label_colors[i]);
				text.scale = PxScale::from(font_size);
				section.add_text(text)
			};
			let bounding_box = match ui_internals.glyph_brush.glyph_bounds(&section) {
				Some(rect) => { rect }
				None => { continue; }
			};

			//Create the associated UI button
			let width = bounding_box.width() + BORDER_WIDTH * 2.0;
            let height = bounding_box.height() + BORDER_WIDTH * 2.0;

            let button_bounds = match self.anchor {
                UIAnchor::LeftAligned((x, y)) => {
                    let x_pos = x;
                    let y_pos = y + i as f32 * (height + BUFFER_DISTANCE);
                    glyph_brush::Rectangle {
                        min: [x_pos, y_pos],
                        max: [x_pos + width, y_pos + height]
                    }
                }
                UIAnchor::DeadCenter(window_size) => {
					let total_menu_height = (height + BUFFER_DISTANCE) * self.button_labels.len() as f32 - BUFFER_DISTANCE;

					let x_pos = (window_size.0 - width) / 2.0;
					let y_pos = (window_size.1 - total_menu_height) / 2.0 + i as f32 * (height + BUFFER_DISTANCE);
                    glyph_brush::Rectangle {
                        min: [x_pos, y_pos],
                        max: [x_pos + width, y_pos + height]
                    }
                }
            };
					
		    section.screen_position = (
			    button_bounds.min[0] + BORDER_WIDTH,
			    button_bounds.min[1] + BORDER_WIDTH
		    );

		    //Finally insert the section into the array
		    let section_id = ui_internals.sections.insert(section);

    		let button = UIButton::new(section_id, button_bounds, self.button_commands[i]);
    		self.ids[i] = ui_internals.add_button(button);
        }
        self.active = true;
    }

    //Remove this menu's data from the arrays of buttons and sections
    pub fn hide(&mut self, ui_internals: &mut UIInternals<'a>) {
        if !self.active { return; }
		for id in self.ids.iter() {
			ui_internals.delete_button(*id);
        }
        self.active = false;
    }

    pub fn toggle(&mut self, ui_internals: &mut UIInternals<'a>) {
        if self.active {
            self.hide(ui_internals);
        } else {
            self.show(ui_internals);
        }
    }
}

//Defines the anchor point of the UI element and how that anchor is configured
pub enum UIAnchor {
    LeftAligned((f32, f32)),			//Parameter is the screen-space position of the top-left corner of the entire menu's bounding box
    DeadCenter((f32, f32))				//Parameter is the screen size in pixels
}

#[derive(PartialEq, Eq, Debug)]
pub enum ButtonState {
    None,
    Highlighted
}