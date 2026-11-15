use super::sftp::RemoteEntry;
use std::collections::{BTreeSet, HashMap};

/// Cached remote directory listing for the currently opened SSH session.
#[derive(Clone)]
pub struct RemoteBrowser {
    pub connection: String,
    /// Listing keyed by remote directory; `""` is the login home.
    pub entries: HashMap<String, Vec<RemoteEntry>>,
    pub current: String,
    pub back: Vec<String>,
    pub loading: BTreeSet<String>,
    pub error: Option<String>,
    pub selected: Option<String>,
}

impl RemoteBrowser {
    pub fn new(connection: String) -> Self {
        Self {
            connection,
            entries: HashMap::new(),
            current: String::new(),
            back: Vec::new(),
            loading: BTreeSet::new(),
            error: None,
            selected: None,
        }
    }
}
