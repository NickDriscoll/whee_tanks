use std::collections::HashMap;

pub type Input = (InputKind, glfw::Action);

#[derive(PartialEq, Eq, Hash, Clone)]
pub enum InputKind {
    Key(glfw::Key),
    Mouse(glfw::MouseButton)
}

//Actions that can be mapped to buttons/keys
#[derive(Debug, Clone, Copy)]
pub enum Command {
    Quit,
    ToggleWireframe,
    RotatePlayerTank(f32),
    MovePlayerTank(f32),
    Fire,
    PauseGame,
    UnPauseGame,
    ToggleMenu(usize),
    AppendToMenuChain(usize, usize),
    MenuChainRollback(usize),
    ToggleFullScreen,
    #[cfg(dev_tools)]
    ToggleCollisionVolumes,
    ToggleBlur,
    StartPlaying,
    ReturnToMainMenu
}

pub fn submit_input_command(input: &Input, command_buffer: &mut Vec<Command>, bindings: &HashMap<Input, Command>) {	
	if let Some(command) = bindings.get(input) {
		command_buffer.push(*command);
	}
}