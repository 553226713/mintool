//! Shared connection settings; session-only forwarding never leaks into SFTP.
use super::model::SshConnection;
use anyhow::{bail, Context, Result};

pub fn connection_arguments(connection: &SshConnection) -> Vec<String> {
    let mut args = connection.extra_args.clone();
    if connection.port != 22 {
        args.extend(["-p".into(), connection.port.to_string()]);
    }
    if let Some(identity) = &connection.identity {
        args.extend(["-i".into(), identity.display().to_string()]);
    }
    args.extend([
        "-o".into(),
        "ServerAliveInterval=30".into(),
        "-o".into(),
        "ServerAliveCountMax=3".into(),
    ]);
    args.extend(super::session::control_options());
    args
}

pub fn sftp_arguments(connection: &SshConnection) -> Result<Vec<String>> {
    let mut result = vec!["-q".into()];
    let arguments = connection_arguments(connection);
    let mut args = arguments.iter();
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "-p" => result.extend(["-P".into(), args.next().context("-p 缺少端口")?.clone()]),
            "-l" => result.extend([
                "-o".into(),
                format!("User={}", args.next().context("-l 缺少用户名")?),
            ]),
            "-o" | "-i" | "-J" | "-F" | "-c" => result.extend([
                argument.clone(),
                args.next()
                    .with_context(|| format!("{argument} 缺少参数"))?
                    .clone(),
            ]),
            "-4" | "-6" | "-C" | "-v" | "-q" => result.push(argument.clone()),
            "-L" | "-R" | "-D" => {
                args.next()
                    .with_context(|| format!("{argument} 缺少转发规则"))?;
            }
            "-t" | "-T" | "-N" | "-f" | "-n" | "-X" | "-Y" | "-x" => {}
            "-A" => result.extend(["-o".into(), "ForwardAgent=yes".into()]),
            "-a" => result.extend(["-o".into(), "ForwardAgent=no".into()]),
            _ if argument.starts_with("-p") && argument.len() > 2 => {
                result.extend(["-P".into(), argument[2..].into()])
            }
            _ if argument.starts_with("-l") && argument.len() > 2 => {
                result.extend(["-o".into(), format!("User={}", &argument[2..])])
            }
            _ if ["-o", "-i", "-J", "-F", "-c"]
                .iter()
                .any(|prefix| argument.starts_with(prefix) && argument.len() > 2) =>
            {
                result.push(argument.clone())
            }
            _ if ["-L", "-R", "-D"]
                .iter()
                .any(|prefix| argument.starts_with(prefix) && argument.len() > 2) => {}
            _ => bail!("SFTP 不支持高级参数 {argument}，请使用 -o 配置连接选项"),
        }
    }
    result.extend([
        "-o".into(),
        "ConnectTimeout=8".into(),
        "-o".into(),
        "StrictHostKeyChecking=accept-new".into(),
        "-o".into(),
        "NumberOfPasswordPrompts=1".into(),
        "-o".into(),
        "LogLevel=ERROR".into(),
    ]);
    if connection.password_value().is_none() {
        result.extend(["-o".into(), "BatchMode=yes".into()]);
    }
    result.push(connection.target());
    Ok(result)
}

pub fn authentication_environment(connection: &SshConnection) -> Vec<(String, String)> {
    match (
        connection.password_value(),
        super::session::askpass_executable(),
    ) {
        (Some(password), Some(executable)) => vec![
            ("SSH_ASKPASS".into(), executable.display().to_string()),
            ("SSH_ASKPASS_REQUIRE".into(), "force".into()),
            ("MINTOOL_ASKPASS".into(), "1".into()),
            ("MINTOOL_ASKPASS_PASSWORD".into(), password.into()),
        ],
        _ => Vec::new(),
    }
}

pub fn parse_arguments(value: &str) -> Result<Vec<String>> {
    shell_words::split(value).context("高级参数引号不匹配")
}
