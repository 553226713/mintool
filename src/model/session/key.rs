use super::ModuleId;

/// Stable identity of an open workspace. Runtime resources (dock, terminals,
/// previews) are keyed by this instead of a bare project id.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SessionKey {
    pub module: ModuleId,
    pub id: String,
}
impl SessionKey {
    pub fn new(module: ModuleId, id: impl Into<String>) -> Self {
        Self {
            module,
            id: id.into(),
        }
    }
    pub fn project(id: impl Into<String>) -> Self {
        Self::new(ModuleId::Projects, id)
    }
    pub fn ssh(id: impl Into<String>) -> Self {
        Self::new(ModuleId::Ssh, id)
    }
    pub fn docker(id: impl Into<String>) -> Self {
        Self::new(ModuleId::Docker, id)
    }
    pub fn database(id: impl Into<String>) -> Self {
        Self::new(ModuleId::Database, id)
    }
}
