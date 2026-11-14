use std::env;

use x11rb::{
    connection::Connection,
    protocol::xproto::{AtomEnum, ConnectionExt as _},
};

/// Reconcile XMODIFIERS with the XIM servers that are actually running.
///
/// Desktop sessions can keep the environment from login while the user switches
/// input methods. GPUI's X11 XIM client reads XMODIFIERS when it starts and will
/// silently fall back to plain keyboard input if that server is no longer present.
pub fn configure_x11_ime() {
    let configured = env::var("XMODIFIERS").ok();
    let Ok(available) = xim_servers() else {
        return;
    };
    let Some(server) = select_xim_server(configured.as_deref(), &available) else {
        return;
    };

    let value = format!("@im={server}");
    if configured.as_deref() != Some(value.as_str()) {
        env::set_var("XMODIFIERS", value);
    }
}

fn select_xim_server(configured: Option<&str>, available: &[String]) -> Option<String> {
    match configured.and_then(|value| value.strip_prefix("@im=")) {
        Some(configured) if available.iter().any(|server| server == configured) => {
            Some(configured.to_string())
        }
        _ => available.first().cloned(),
    }
}

fn xim_servers() -> anyhow::Result<Vec<String>> {
    let (connection, screen) = x11rb::connect(None)?;
    let root = connection.setup().roots[screen].root;
    let property = connection.intern_atom(false, b"XIM_SERVERS")?.reply()?.atom;
    let reply = connection
        .get_property(false, root, property, AtomEnum::ATOM, 0, u32::MAX)?
        .reply()?;
    let Some(servers) = reply.value32() else {
        return Ok(Vec::new());
    };

    Ok(servers
        .filter_map(|atom| connection.get_atom_name(atom).ok())
        .filter_map(|cookie| cookie.reply().ok())
        .filter_map(|reply| String::from_utf8(reply.name).ok())
        .filter_map(|name| {
            name.strip_prefix("@server=")
                .filter(|server| !server.is_empty())
                .map(str::to_owned)
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::select_xim_server;

    #[test]
    fn stale_environment_falls_back_to_a_running_input_method() {
        assert_eq!(
            select_xim_server(Some("@im=ibus"), &["fcitx".into()]),
            Some("fcitx".into())
        );
    }

    #[test]
    fn active_environment_is_preserved() {
        assert_eq!(
            select_xim_server(Some("@im=fcitx"), &["ibus".into(), "fcitx".into()]),
            Some("fcitx".into())
        );
    }

    #[test]
    fn no_running_server_leaves_input_unchanged() {
        assert_eq!(select_xim_server(Some("@im=ibus"), &[]), None);
    }
}
