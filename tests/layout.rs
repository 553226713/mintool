use mintool::workspace::dock_layout::DockLayout;
use mintool::workspace::layout::{Axis, Layout};

#[test]
fn new_file_tabs_follow_the_selected_tab_in_its_split_group() {
    let mut layout = DockLayout::Split {
        axis: Axis::Horizontal,
        sizes: vec![400., 800.],
        children: vec![
            DockLayout::Tabs {
                panes: vec![1, 3, 4],
                active: 0,
            },
            DockLayout::Tabs {
                panes: vec![2, 5],
                active: 0,
            },
        ],
    };
    assert!(layout.add_tab_after(1, 6));
    assert!(layout.add_tab_after(2, 7));
    assert!(!layout.add_tab_after(99, 8));
    let restored: DockLayout =
        serde_json::from_str(&serde_json::to_string(&layout).unwrap()).unwrap();
    let DockLayout::Split { children, .. } = &restored else {
        panic!("expected split groups");
    };
    for (group, expected) in children.iter().zip([vec![1, 6, 3, 4], vec![2, 7, 5]]) {
        let DockLayout::Tabs { panes, active } = group else {
            panic!("expected tabs");
        };
        assert_eq!(panes, &expected);
        assert_eq!(*active, 1);
    }
    assert!(restored.valid_for(&[1, 2, 3, 4, 5, 6, 7]));
}

#[test]
fn dock_tabs_preserve_order_active_tab_and_all_pane_ids_on_reload() {
    let layout = DockLayout::Split {
        axis: Axis::Horizontal,
        sizes: vec![400., 800.],
        children: vec![
            DockLayout::Tabs {
                panes: vec![3, 1],
                active: 1,
            },
            DockLayout::Tabs {
                panes: vec![2],
                active: 0,
            },
        ],
    };
    let restored: DockLayout =
        serde_json::from_str(&serde_json::to_string(&layout).unwrap()).unwrap();
    assert!(restored.valid_for(&[1, 2, 3]));
    let mut ids = vec![];
    restored.ids(&mut ids);
    assert_eq!(ids, [3, 1, 2]);
    assert!(!restored.valid_for(&[1, 2]));
    assert!(!DockLayout::Tabs {
        panes: vec![1, 1],
        active: 0
    }
    .valid_for(&[1]));
    assert!(!DockLayout::Tabs {
        panes: vec![1],
        active: 1
    }
    .valid_for(&[1]));
    assert!(!DockLayout::Split {
        axis: Axis::Vertical,
        sizes: vec![-1.],
        children: vec![DockLayout::Tabs {
            panes: vec![1],
            active: 0
        }]
    }
    .valid_for(&[1]));
}

fn collect_heights(layout: &Layout, height: f32, heights: &mut Vec<f32>) {
    match layout {
        Layout::Pane(_) => heights.push(height),
        Layout::Split {
            axis: Axis::Vertical,
            ratio,
            first,
            second,
        } => {
            collect_heights(first, height * ratio, heights);
            collect_heights(second, height * (1. - ratio), heights);
        }
        _ => panic!("expected a vertical group"),
    }
}

#[test]
fn repeated_splits_share_space_without_shrinking_the_first_terminal() {
    let mut layout = Layout::pane(1);
    layout.split(1, 2, Axis::Horizontal);
    for id in 3..=7 {
        assert!(layout.split(1, id, Axis::Vertical));
    }
    let Layout::Split {
        ratio,
        first,
        second,
        ..
    } = &layout
    else {
        panic!()
    };
    assert_eq!(*ratio, 0.5);
    assert_eq!(second.first_id(), 2);
    let mut heights = Vec::new();
    collect_heights(first, 840., &mut heights);
    assert_eq!(heights.len(), 6);
    assert!(heights.iter().all(|height| (*height - 140.).abs() < 0.001));
    assert_eq!(layout.minimum_size(), (480., 840.));
}

#[test]
fn balancing_stops_at_a_different_split_direction() {
    let mut layout = Layout::pane(1);
    layout.split(1, 2, Axis::Vertical);
    layout.set_ratio(1, 2, 0.7);
    layout.split(1, 3, Axis::Horizontal);
    layout.split(1, 4, Axis::Vertical);
    layout.split(1, 5, Axis::Vertical);
    let Layout::Split { ratio, first, .. } = &layout else {
        panic!()
    };
    assert_eq!(*ratio, 0.7);
    let Layout::Split { ratio, first, .. } = first.as_ref() else {
        panic!()
    };
    assert_eq!(*ratio, 0.5);
    let mut heights = Vec::new();
    collect_heights(first, 600., &mut heights);
    assert!(heights.iter().all(|height| (*height - 200.).abs() < 0.001));
}
