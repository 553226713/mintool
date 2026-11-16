//! POSIX-style remote path helpers shared by SFTP and the remote explorer.

pub fn join_remote(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_string()
    } else {
        format!("{}/{}", parent.trim_end_matches('/'), name)
    }
}

pub fn parent_of(path: &str) -> &str {
    let parent = path
        .trim_end_matches('/')
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .unwrap_or("");
    if parent.is_empty() && path.starts_with('/') {
        "/"
    } else {
        parent
    }
}

pub fn basename(path: &str) -> &str {
    path.trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(path)
}

pub fn directory_command(path: &str) -> String {
    if path.is_empty() {
        "cd".into()
    } else {
        format!("cd -- '{}'", path.replace('\'', "'\\''"))
    }
}
