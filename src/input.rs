#[derive(PartialEq, Eq, Hash)]
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
    MoveForwards,
    MoveBackwards,
    StopMoving,
    StopRotating,
    Fire,
    ToggleDebugMenu,
    TogglePauseMenu
}