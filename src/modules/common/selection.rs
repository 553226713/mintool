use std::collections::BTreeSet;

pub fn select<K: Clone + Ord>(
    ordered: &[K],
    selected: &mut BTreeSet<K>,
    anchor: Option<&K>,
    target: K,
    toggle: bool,
    extend: bool,
) {
    if extend {
        if let Some((a, b)) = anchor
            .and_then(|anchor| ordered.iter().position(|x| x == anchor))
            .zip(ordered.iter().position(|x| x == &target))
        {
            if !toggle {
                selected.clear();
            }
            selected.extend(ordered[a.min(b)..=a.max(b)].iter().cloned());
            return;
        }
    }
    if toggle {
        if !selected.remove(&target) {
            selected.insert(target);
        }
    } else {
        selected.clear();
        selected.insert(target);
    }
}
