use crate::input::{Command};
use gl::types::*;
use ozy_engine::structs::{OptionVec};
use ozy_engine::glutil;
use glyph_brush::{GlyphBrush, GlyphCruncher, ab_glyph::PxScale, Section, Text};
use std::{mem, ptr};
use std::os::raw::c_void;

type GlyphBrushVertexType = [f32; 16];

pub struct UISystem<'a> {
    pub glyph_brush: GlyphBrush<GlyphBrushVertexType>,
    pub buttons: OptionVec<UIButton>,
    pub sections: OptionVec<Section<'a>>,
    pub button_vao: Option<GLuint>,
    pub glyph_vao: Option<GLuint>,
    pub glyph_texture: GLuint,
    pub glyph_count: usize,
    pub button_color_instanced_buffer: GLuint,
    pub update: bool
}

impl<'a> UISystem<'a> {
    const FLOATS_PER_COLOR: usize = 4;
    const COLORS_PER_BUTTON: usize = 4;

    pub unsafe fn new(glyph_brush: GlyphBrush<GlyphBrushVertexType>) -> Self {
        //Create the glyph texture
        let glyph_texture = unsafe {
            let (width, height) = glyph_brush.texture_dimensions();
            let mut tex = 0;
            gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);
            gl::GenTextures(1, &mut tex);
            gl::BindTexture(gl::TEXTURE_2D, tex);
            let params = (
                gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE,
                gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE,
                gl::TEXTURE_MIN_FILTER, gl::LINEAR,
                gl::TEXTURE_MAG_FILTER, gl::LINEAR
            );
            glutil::apply_texture_parameters(&params);
            gl::TexImage2D(gl::TEXTURE_2D, 0, gl::RED as GLint, width as GLint, height as GLint, 0, gl::RED, gl::UNSIGNED_BYTE, ptr::null());
            tex
        };
        UISystem {
            glyph_brush,
            buttons: OptionVec::new(),
            sections: OptionVec::new(),
            button_vao: None,
            glyph_vao: None,
            glyph_count: 0,
            button_color_instanced_buffer: 0,
            update: false
        }
    }

    pub unsafe fn update_button_color(&self, index: usize, color: [f32; Self::FLOATS_PER_COLOR]) {
        let mut data = vec![0.0; Self::FLOATS_PER_COLOR * Self::COLORS_PER_BUTTON];
        for i in 0..(data.len() / Self::FLOATS_PER_COLOR) {
            data[i * Self::FLOATS_PER_COLOR] = color[0];
            data[i * Self::FLOATS_PER_COLOR + 1] = color[1];
            data[i * Self::FLOATS_PER_COLOR + 2] = color[2];
            data[i * Self::FLOATS_PER_COLOR + 3] = color[3];
        }
        gl::BindBuffer(gl::ARRAY_BUFFER, self.button_color_instanced_buffer);
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
    labels: Vec<&'a str>,
    commands: Vec<Option<Command>>,
    anchor: UIAnchor,
    active: bool,
    ids: Vec<usize>
}

impl<'a> Menu<'a> {
    pub fn new(labels: Vec<&'a str>, commands: Vec<Option<Command>>, anchor: UIAnchor) -> Self {
        if labels.len() != commands.len() {
            panic!("Tried to create menu with non-matching labels and commands sizes");
        }

        let size = labels.len();
        Menu {
            labels,
            commands,
            anchor,
            active: false,
            ids: vec![0; size]
        }
    }

    //Adds this menu's data to the arrays of buttons and sections
    fn show<'b>(&mut self, ui_system: &mut UISystem<'a>) {
        //Submit the pause menu data
		const BORDER_WIDTH: f32 = 15.0;
		const GAP_DISTANCE: f32 = 10.0;
		let font_size = 36.0;
		for i in 0..self.labels.len() {
			let mut section = {
				let section = Section::new();
				let mut text = Text::new(self.labels[i]).with_color([1.0, 1.0, 1.0, 1.0]);
				text.scale = PxScale::from(font_size);
				section.add_text(text)
			};
			let bounding_box = match ui_system.glyph_brush.glyph_bounds(&section) {
				Some(rect) => { rect }
				None => { continue; }
            };

			//Create the associated UI button
			let width = bounding_box.width() + BORDER_WIDTH * 2.0;
            let height = bounding_box.height() + BORDER_WIDTH * 2.0;
            let button_bounds = match self.anchor {
                UIAnchor::LeftAligned(x, y) => {
                    let x_pos = x;
                    let y_pos = y + i as f32 * (height + GAP_DISTANCE);
                    glyph_brush::Rectangle {
                        min: [x_pos, y_pos],
                        max: [x_pos + width, y_pos + height]
                    }
                }
                UIAnchor::CenterAligned(x, y) => {
                    let x_pos = x - width / 2.0;
                    let y_pos = y + i as f32 * (height + GAP_DISTANCE);
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
		    let section_id = ui_system.sections.insert(section);

    		let button = UIButton::new(section_id, button_bounds, self.commands[i]);
    		self.ids[i] = ui_system.buttons.insert(button);
        }
        self.active = true;
    }

    //Remove this menu's data from the arrays of buttons and sections
    fn hide(&mut self, ui_system: &mut UISystem<'a>) {
		for id in self.ids.iter() {
			if let Some(button) = &ui_system.buttons[*id] {
                ui_system.sections.delete(button.section_id());
                ui_system.buttons.delete(*id);
			}
        }
        self.active = false;
    }

    pub fn toggle<'b>(&mut self, ui_system: &mut UISystem<'a>) {
        if self.active {
            self.hide(ui_system);
        } else {
            self.show(ui_system);
        }
    }
}

pub enum UIAnchor {
    LeftAligned(f32, f32),
    CenterAligned(f32, f32)
}

#[derive(PartialEq, Eq, Debug)]
pub enum ButtonState {
    None,
    Highlighted
}