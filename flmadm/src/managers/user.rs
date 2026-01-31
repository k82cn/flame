pub struct UserManager;

impl UserManager {
    pub fn new() -> Self {
        Self
    }

    /// Check if we're running as root
    pub fn is_root(&self) -> bool {
        users::get_current_uid() == 0
    }
}
