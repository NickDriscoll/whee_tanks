use crate::input::{Command};
use ozy_engine::structs::{OptionVec};
use glyph_brush::{GlyphBrush, GlyphCruncher, ab_glyph::PxScale, Section, Text};

type GlyphBrushVertexType = [f32; 16];

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
    anchor: MenuAnchor,
    active: bool,
    ids: Vec<usize>
}

impl<'a> Menu<'a> {
    pub fn new(labels: Vec<&'a str>, commands: Vec<Option<Command>>, anchor: MenuAnchor) -> Self {
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
    pub fn show<'b>(&mut self, ui_buttons: &mut OptionVec<UIButton>, sections: &'b mut OptionVec<Section<'a>>, glyph_brush: &mut GlyphBrush<GlyphBrushVertexType>) {
        //Submit the pause menu text
		const BORDER_WIDTH: f32 = 15.0;
		const BUFFER_DISTANCE: f32 = 10.0;
		let font_size = 36.0;
		for i in 0..self.labels.len() {
			let mut section = {
				let section = Section::new();
				let mut text = Text::new(self.labels[i]).with_color([1.0, 1.0, 1.0, 1.0]);
				text.scale = PxScale::from(font_size);
				section.add_text(text)
			};
			let bounding_box = match glyph_brush.glyph_bounds(&section) {
				Some(rect) => { rect }
				None => { continue; }
			};

			//Create the associated UI button
			let width = bounding_box.width() + BORDER_WIDTH * 2.0;
            let height = bounding_box.height() + BORDER_WIDTH * 2.0;

            let button_bounds = match self.anchor {
                MenuAnchor::LeftAligned(x, y) => {
                    let y_pos = y + i as f32 * (height + BUFFER_DISTANCE);
                    glyph_brush::Rectangle {
                        min: [x, y_pos],
                        max: [x + width, y_pos + height]
                    }
                }
                MenuAnchor::CenterAligned(x, y) => {
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
		    let section_id = sections.insert(section);

    		let button = UIButton::new(section_id, button_bounds, self.commands[i]);
    		self.ids[i] = ui_buttons.insert(button);
        }
        self.active = true;
    }

    //Remove this menu's data from the arrays of buttons and sections
    pub fn hide(&mut self, ui_buttons: &mut OptionVec<UIButton>, sections: &mut OptionVec<Section>) {
		for id in self.ids.iter() {
			if let Some(button) = &ui_buttons[*id] {
                sections.delete(button.section_id());
                ui_buttons.delete(*id);
			}
        }
        self.active = false;
    }

    pub fn toggle<'b>(&mut self, ui_buttons: &mut OptionVec<UIButton>, sections: &'b mut OptionVec<Section<'a>>, glyph_brush: &mut GlyphBrush<GlyphBrushVertexType>) {
        if self.active {
            self.hide(ui_buttons, sections);
        } else {
            self.show(ui_buttons, sections, glyph_brush);
        }
    }
}

pub enum MenuAnchor {
    LeftAligned(f32, f32),
    CenterAligned(f32, f32)
}

#[derive(PartialEq, Eq, Debug)]
pub enum ButtonState {
    None,
    Highlighted
}