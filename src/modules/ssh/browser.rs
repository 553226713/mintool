use super::sftp::RemoteEntry;
use std::collections::{BTreeSet, HashMap};

/// Cached remote directory listing for the currently opened SSH session.
#[derive(Clone)]
pub struct RemoteBrowser {
    pub connection: String,
    /// Listing keyed by remote directory; `""` is the login home.
    pub entries: HashMap<String, Vec<RemoteEntry>>,
    pub current: String,
    pub home: Option<String>,
    pub back: Vec<String>,
    pub forward: Vec<String>,
    pub loading: BTreeSet<String>,
    pub error: Option<String>,
    pub selected: Option<String>,
    pub selected_paths: std::collections::BTreeSet<String>,
}

impl RemoteBrowser {
    pub fn new(connection: String) -> Self {
        Self {
            connection,
            entries: HashMap::new(),
            current: String::new(),
            home: None,
            back: Vec::new(),
            forward: Vec::new(),
            loading: BTreeSet::new(),
            error: None,
            selected: None,
            selected_paths: Default::default(),
        }
    }
}
