use mintool::model::settings::Settings;
use mintool::modules::projects::model::ProjectRegistry;
use std::fs;

#[test]
fn adding_projects_selects_a_group_without_reassigning_existing_projects() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("project")).unwrap();
    let mut registry = ProjectRegistry::default();
    let work = registry.add_group("Work").unwrap();
    let personal = registry.add_group("Personal").unwrap();
    let id = registry
        .add_in_group(root.path().join("project"), Some(work.clone()))
        .unwrap();
    assert_eq!(registry.projects[0].group.as_ref(), Some(&work));
    assert_eq!(
        registry
            .add_in_group(root.path().join("project"), Some(personal))
            .unwrap(),
        id
    );
    assert_eq!(registry.projects.len(), 1);
    assert_eq!(registry.projects[0].group.as_ref(), Some(&work));
    assert!(registry.add_group(" Work ").is_err());
    assert!(registry.add_group("   ").is_err());
    assert!(registry
        .add_in_group(root.path().into(), Some("missing".into()))
        .is_err());
    assert_eq!(registry.projects.len(), 1);
}

#[test]
fn menu_reordering_stays_within_the_projects_group_and_handles_boundaries() {
    let root = tempfile::tempdir().unwrap();
    let mut registry = ProjectRegistry::default();
    let group = registry.add_group("Work").unwrap();
    let second_group = registry.add_group("Personal").unwrap();
    let mut ids = Vec::new();
    for (name, group) in [
        ("a", Some(group.clone())),
        ("other", None),
        ("b", Some(group)),
    ] {
        let path = root.path().join(name);
        fs::create_dir(&path).unwrap();
        ids.push(registry.add_in_group(path, group).unwrap());
    }
    registry.shift_project(&ids[2], -1);
    assert_eq!(
        registry
            .projects
            .iter()
            .map(|p| p.id.clone())
            .collect::<Vec<_>>(),
        vec![ids[2].clone(), ids[1].clone(), ids[0].clone()]
    );
    registry.shift_project(&ids[2], -1);
    registry.shift_project(&ids[1], 1);
    assert_eq!(registry.projects[0].id, ids[2]);
    assert_eq!(registry.projects[1].id, ids[1]);
    registry.shift_group(&second_group, -1);
    registry.shift_group(&second_group, -1);
    assert_eq!(registry.groups[0].id, second_group);
}

#[test]
fn narrow_windows_reserve_workspace_space_and_keep_saved_sidebar_preferences() {
    let settings = Settings {
        left_width: 323.,
        right_width: 475.,
        ..Settings::default()
    };
    for width in [950., 1024., 1280., 1440., 1920., 2560.] {
        let (left, right) = settings.panel_widths(width);
        assert!(left >= 180. && right >= 200.);
        assert!(width - left - right >= 400.);
    }
    assert_eq!(settings.panel_widths(1920.), (323., 475.));
    assert_eq!((settings.left_width, settings.right_width), (323., 475.));
}
