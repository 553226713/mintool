use mintool::services::directory_browser::DirectoryBrowser;
use std::{fs, path::Path};

#[test]
fn directory_picker_lists_hidden_directories_and_follows_directory_links() {
    let root = tempfile::tempdir().unwrap();
    for name in ["folder with spaces", ".hidden", "target"] {
        fs::create_dir(root.path().join(name)).unwrap();
    }
    fs::write(root.path().join("file"), "text").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        root.path().join("target"),
        root.path().join("linked directory"),
    )
    .unwrap();
    let browser = DirectoryBrowser::open(root.path()).unwrap();
    let names: Vec<_> = browser
        .directories
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert!(names.contains(&".hidden".to_string()));
    assert!(names.contains(&"folder with spaces".to_string()));
    assert!(!names.contains(&"file".to_string()));
    assert!(browser
        .files
        .iter()
        .any(|path| path.file_name().unwrap() == "file"));
    #[cfg(unix)]
    {
        assert!(names.contains(&"linked directory".to_string()));
        assert_eq!(
            browser.resolve("linked directory").unwrap(),
            root.path().join("target").canonicalize().unwrap()
        );
    }
    assert_eq!(
        browser.resolve("folder with spaces").unwrap(),
        root.path()
            .join("folder with spaces")
            .canonicalize()
            .unwrap()
    );
    assert_eq!(
        browser.resolve_file("file").unwrap(),
        root.path().join("file").canonicalize().unwrap()
    );
    assert_eq!(
        browser.resolve_new_file("new.sqlite").unwrap(),
        root.path().canonicalize().unwrap().join("new.sqlite")
    );
}

#[test]
fn invalid_paths_report_errors_without_changing_the_browsed_directory() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("file"), "text").unwrap();
    let browser = DirectoryBrowser::open(root.path()).unwrap();
    assert!(browser.resolve("missing").is_err());
    assert!(browser.resolve("file").is_err());
    assert!(browser.resolve("").is_err());
    assert!(DirectoryBrowser::open(&root.path().join("file")).is_err());
    assert_eq!(browser.path, root.path().canonicalize().unwrap());
}

#[test]
fn repository_directory_is_visible_from_its_parent_even_on_a_mounted_workspace() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .canonicalize()
        .unwrap();
    let browser = DirectoryBrowser::open(repository.parent().unwrap()).unwrap();
    assert!(browser
        .directories
        .iter()
        .any(|p| p.canonicalize().ok().as_ref() == Some(&repository)));
    assert_eq!(
        browser
            .resolve(repository.to_string_lossy().as_ref())
            .unwrap(),
        repository
    );
}
