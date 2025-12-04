//! Keyboard and mouse input handling

/// Actions that can be triggered by user input
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputAction {
    /// Go back in path history (h, left, backspace)
    GoBack,
    /// Move down in edge selection (j, down)
    MoveDown,
    /// Move up in edge selection (k, up)
    MoveUp,
    /// Follow selected edge (l, right)
    MoveRight,
    /// Jump directly to edge N (1-9)
    SelectEdge(usize),
    /// Reset to entry block (Escape)
    Reset,
    /// Focus the function search/selector (/)
    FocusSearch,
    /// No action
    None,
}

/// Parse a key string into an action
pub fn parse_key(key: &str) -> InputAction {
    match key {
        // Vim-style and arrow navigation
        "h" | "ArrowLeft" | "Backspace" => InputAction::GoBack,
        "j" | "ArrowDown" => InputAction::MoveDown,
        "k" | "ArrowUp" => InputAction::MoveUp,
        "l" | "ArrowRight" | "Enter" => InputAction::MoveRight,

        // Reset
        "Escape" => InputAction::Reset,

        // Search focus
        "/" => InputAction::FocusSearch,

        // Number keys for direct edge selection (1-indexed for UX)
        "1" => InputAction::SelectEdge(0),
        "2" => InputAction::SelectEdge(1),
        "3" => InputAction::SelectEdge(2),
        "4" => InputAction::SelectEdge(3),
        "5" => InputAction::SelectEdge(4),
        "6" => InputAction::SelectEdge(5),
        "7" => InputAction::SelectEdge(6),
        "8" => InputAction::SelectEdge(7),
        "9" => InputAction::SelectEdge(8),

        _ => InputAction::None,
    }
}
