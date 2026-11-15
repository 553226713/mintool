#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SessionCapabilities {
    pub files: bool,
    pub git: bool,
    pub explorer: bool,
}
impl SessionCapabilities {
    pub const PROJECT: Self = Self {
        files: true,
        git: true,
        explorer: true,
    };
    pub const SSH: Self = Self {
        files: false,
        git: false,
        explorer: false,
    };
    pub const DOCKER: Self = Self {
        files: false,
        git: false,
        explorer: false,
    };
    pub const DATABASE: Self = Self {
        files: false,
        git: false,
        explorer: false,
    };
}
