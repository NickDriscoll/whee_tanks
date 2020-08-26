use crate::input::{Command};
use ozy_engine::structs::{OptionVec};
use glyph_brush::{GlyphBrush, GlyphCruncher, ab_glyph::PxScale, Section, Text};
use gl::types::*;
use std::os::raw::c_void;
use std::mem;

type GlyphBrushVertexType = [f32; 16];

pub struct UIState<'a> {
    pub glyph_brush: &'a mut GlyphBrush<GlyphBrushVertexType>,
    pub button_color_buffer: GLuint,
    pub buttons: OptionVec<UIButton>,
    pub sections: OptionVec<Section<'a>>,
}
impl<'a> UIState<'a> {
    const FLOATS_PER_COLOR: usize = 4;
    const COLORS_PER_BUTTON: usize = 4;

    pub fn new(glyph_brush: &'a mut GlyphBrush<GlyphBrushVertexType>) -> Self {
        UIState {
            glyph_brush,
            button_color_buffer: 0,
            buttons: OptionVec::new(),
            sections: OptionVec::new()
        }
    }

    //Gets input from the UI system and generates Commands for the command buffer I.E. user clicking on buttons
    //Also updates the instanced color buffer used for rendering the buttons
    pub fn button_update(&mut self, screen_space_mouse: glm::TVec2<f32>, mouse_lbutton_pressed: bool, mouse_lbutton_pressed_last_frame: bool, command_buffer: &mut Vec<Command>) {        
		//Handle input from the UI buttons
		let mut current_button = 0;
		for i in 0..self.buttons.len() {
			if let Some(button) = self.buttons.get_mut_element(i) {
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

    pub fn section_id(&self) -> usize {
        self.section_id
    }
}

pub struct Menu<'a> {
    buttons: Vec<(&'a str, Option<Command>)>,
    anchor: UIAnchor,
    active: bool,
    ids: Vec<usize>
}

impl<'a> Menu<'a> {
    pub fn new(buttons: Vec<(&'a str, Option<Command>)>, anchor: UIAnchor) -> Self {
        let size = buttons.len();
        Menu {
            buttons,
            anchor,
            active: false,
            ids: vec![0; size]
        }
    }

    //Adds this menu's data to the arrays of buttons and sections
    pub fn show<'b>(&mut self, ui_state: &mut UIState<'a>) {
        if self.active { return; }

        //Submit the pause menu data
		const BORDER_WIDTH: f32 = 15.0;
		const BUFFER_DISTANCE: f32 = 10.0;
		let font_size = 36.0;
		for i in 0..self.buttons.len() {
			let mut section = {
				let section = Section::new();
				let mut text = Text::new(self.buttons[i].0).with_color([1.0, 1.0, 1.0, 1.0]);
				text.scale = PxScale::from(font_size);
				section.add_text(text)
			};
			let bounding_box = match ui_state.glyph_brush.glyph_bounds(&section) {
				Some(rect) => { rect }
				None => { continue; }
			};

			//Create the associated UI button
			let width = bounding_box.width() + BORDER_WIDTH * 2.0;
            let height = bounding_box.height() + BORDER_WIDTH * 2.0;

            let button_bounds = match self.anchor {
                UIAnchor::LeftAligned(x, y) => {
                    let x_pos = x;
                    let y_pos = y + i as f32 * (height + BUFFER_DISTANCE);
                    glyph_brush::Rectangle {
                        min: [x_pos, y_pos],
                        max: [x_pos + width, y_pos + height]
                    }
                }
                UIAnchor::CenterAligned(x, y) => {
                    let x_pos = x - width / 2.0;
                    let y_pos = y + i as f32 * (height + BUFFER_DISTANCE);
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
		    let section_id = ui_state.sections.insert(section);

    		let button = UIButton::new(section_id, button_bounds, self.buttons[i].1);
    		self.ids[i] = ui_state.buttons.insert(button);
        }
        self.active = true;
    }

    //Remove this menu's data from the arrays of buttons and sections
    pub fn hide(&mut self, ui_state: &mut UIState) {
        if !self.active { return; }
		for id in self.ids.iter() {
			if let Some(button) = &ui_state.buttons[*id] {
                ui_state.sections.delete(button.section_id());
                ui_state.buttons.delete(*id);
			}
        }
        self.active = false;
    }

    pub fn toggle<'b>(&mut self, ui_state: &mut UIState<'a>) {
        if self.active {
            self.hide(ui_state);
        } else {
            self.show(ui_state);
        }
    }
}

//Defines the anchor point of the UI element and how that anchor is configured
pub enum UIAnchor {
    LeftAligned(f32, f32),
    CenterAligned(f32, f32)
}

#[derive(PartialEq, Eq, Debug)]
pub enum ButtonState {
    None,
    Highlighted
}