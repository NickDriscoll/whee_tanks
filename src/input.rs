#[derive(PartialEq, Eq, Hash, Clone)]
pub enum InputType {
    Key(glfw::Key),
    Mouse(glfw::MouseButton)
}

//Actions that can be mapped to buttons/keys
#[derive(Debug, Clone, Copy)]
pub enum Command {
    Quit,
    ToggleWireframe,
    RotateLeft,
    RotateRight,
    StopRotateLeft,
    StopRotateRight,
    MoveForwards,
    MoveBackwards,
    StopMoving,
    Fire,
    TogglePauseMenu,
    StartPlaying,
    ReturnToMainMenu
}