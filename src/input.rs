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
    RotateLeft,
    RotateRight,
    StopRotateLeft,
    StopRotateRight,
    MoveForwards,
    MoveBackwards,
    StopMoveForwards,
    StopMoveBackwards,
    Fire,
    TogglePauseMenu,
    //ToggleFullScreen,
    StartPlaying,
    ReturnToMainMenu
}