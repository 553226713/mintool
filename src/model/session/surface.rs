#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SessionSurface {
    #[default]
    Terminal,
    Query,
}
