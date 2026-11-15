#[derive(Clone, Debug)]
pub struct SshLaunch {
    pub program: String,
    pub args: Vec<String>,
    pub password: Option<String>,
    /// Environment prepared by the SSH module (for example `SSH_ASKPASS`).
    pub envs: Vec<(String, String)>,
}
