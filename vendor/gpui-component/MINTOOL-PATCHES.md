# Mintool integration

Source: crates.io `gpui-component` 0.5.1, Apache-2.0 (see LICENSE).

The drag/drop docking implementation is reused unchanged. The local changes bind `gpui`
and `gpui-macros` to the existing Adabraka 0.5.1 packages, initialize Adabraka's
additional shadow/quad fields to their defaults, and convert shadow vectors into
the small-vector representation expected by this fork. Keep these changes when
updating the vendored library.

`DockItem::new_tabs` is made public to restore the active index while building
tabs from Mintool's persisted pane IDs. The matching assets package is also
vendored with its GPUI dependency bound to Adabraka.

The active panel uses normal view rendering inside a relative, clipped flex
container. The upstream cached absolute view produced empty/clipped terminal
surfaces with this Adabraka fork. Native terminal sessions must also recompute
their child-window bounds when a panel moves. Drag/drop behavior is unchanged.

Tab headers handle the middle mouse button by removing the clicked closable
panel through the existing `TabPanel::remove_panel` path. Mintool supplies its
own per-tab close button and final-terminal placeholder.
