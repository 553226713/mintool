use super::model::{DockerContainer, DockerImage, DockerNetwork, DockerVolume};
use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::process::Command;

pub fn binary() -> String {
    std::env::var("MINTOOL_DOCKER_BIN").unwrap_or_else(|_| "docker".into())
}

fn timeout() -> &'static str {
    if std::path::Path::new("/usr/bin/timeout").exists() {
        "/usr/bin/timeout"
    } else {
        "timeout"
    }
}

fn run(args: &[&str], seconds: &str) -> Result<String> {
    let output = Command::new(timeout())
        .arg(seconds)
        .arg(binary())
        .args(args)
        .output()
        .context("无法启动 docker 命令，请确认已安装 Docker")?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    if output.status.code() == Some(124) {
        bail!("Docker 操作超时");
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let mut message = if !stderr.is_empty() { stderr } else { stdout };
    if message.contains("Cannot connect to the Docker daemon")
        || message.contains("permission denied")
    {
        message.push_str("（请确认 Docker 服务已启动，并检查当前用户是否有 Docker socket 权限）");
    }
    bail!(
        "{}",
        message.lines().take(4).collect::<Vec<_>>().join(" / ")
    )
}

pub fn list_containers() -> Result<Vec<DockerContainer>> {
    let output = run(&["ps", "-a", "--format", "{{json .}}"], "10")?;
    Ok(parse_containers(&output))
}

pub fn list_images() -> Result<Vec<DockerImage>> {
    let output = run(&["images", "--format", "{{json .}}"], "10")?;
    Ok(parse_images(&output))
}

pub fn list_volumes() -> Result<Vec<DockerVolume>> {
    let output = run(&["volume", "ls", "--format", "{{json .}}"], "10")?;
    Ok(parse_volumes(&output))
}

pub fn list_networks() -> Result<Vec<DockerNetwork>> {
    let output = run(&["network", "ls", "--format", "{{json .}}"], "10")?;
    Ok(parse_networks(&output))
}

pub fn snapshot() -> Result<(
    Vec<DockerContainer>,
    Vec<DockerImage>,
    Vec<DockerVolume>,
    Vec<DockerNetwork>,
)> {
    Ok((
        list_containers()?,
        list_images()?,
        list_volumes()?,
        list_networks()?,
    ))
}

pub fn action(id: &str, action: &str) -> Result<()> {
    run(&[action, id], "20").map(|_| ())
}

pub fn remove_container(id: &str) -> Result<()> {
    run(&["rm", "-f", id], "20").map(|_| ())
}

pub fn remove_image(id: &str) -> Result<()> {
    run(&["image", "rm", id], "20").map(|_| ())
}

pub fn remove_volume(name: &str) -> Result<()> {
    run(&["volume", "rm", name], "20").map(|_| ())
}

pub fn remove_network(id: &str) -> Result<()> {
    run(&["network", "rm", id], "20").map(|_| ())
}

pub fn logs_spec(id: &str) -> Vec<String> {
    vec![
        "logs".to_string(),
        "-f".to_string(),
        "--tail".to_string(),
        "500".to_string(),
        id.to_string(),
    ]
}

pub fn exec_spec(id: &str) -> Vec<String> {
    vec![
        "exec".to_string(),
        "-it".to_string(),
        id.to_string(),
        "/bin/sh".to_string(),
    ]
}

pub fn parse_containers(output: &str) -> Vec<DockerContainer> {
    let mut containers = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let id = text(&value, "ID");
        if id.is_empty() {
            continue;
        }
        let names = text(&value, "Names");
        let mut name = names
            .split(',')
            .next()
            .unwrap_or(&names)
            .trim_start_matches('/')
            .to_string();
        if name.is_empty() {
            name = id.clone();
        }
        containers.push(DockerContainer {
            id,
            name,
            image: text(&value, "Image"),
            state: text(&value, "State").to_ascii_lowercase(),
            status: text(&value, "Status"),
            ports: text(&value, "Ports"),
        });
    }
    containers.sort_by(|left, right| {
        right
            .active()
            .cmp(&left.active())
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });
    containers
}

pub fn parse_images(output: &str) -> Vec<DockerImage> {
    let mut images = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let repository = text(&value, "Repository");
        images.push(DockerImage {
            id: text(&value, "ID"),
            repository: repository.clone(),
            tag: text(&value, "Tag"),
            size: text(&value, "Size"),
        });
    }
    images.sort_by_cached_key(|image| image.reference());
    images
}

pub fn parse_volumes(output: &str) -> Vec<DockerVolume> {
    let mut volumes = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let name = text(&value, "Name");
        if name.is_empty() {
            continue;
        }
        volumes.push(DockerVolume {
            name,
            driver: text(&value, "Driver"),
            mountpoint: text(&value, "Mountpoint"),
        });
    }
    volumes.sort_by_cached_key(|volume| volume.name.to_lowercase());
    volumes
}

pub fn parse_networks(output: &str) -> Vec<DockerNetwork> {
    let mut networks = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let id = text(&value, "ID");
        let name = text(&value, "Name");
        if id.is_empty() || name.is_empty() {
            continue;
        }
        networks.push(DockerNetwork {
            id,
            name,
            driver: text(&value, "Driver"),
            scope: text(&value, "Scope"),
        });
    }
    networks.sort_by_cached_key(|network| network.name.to_lowercase());
    networks
}

fn text(value: &Value, field: &str) -> String {
    value
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::{parse_containers, parse_images, parse_networks, parse_volumes};

    #[test]
    fn parses_container_json_and_sorts_running_first() {
        let output = r#"{"ID":"aaa","Names":"web","Image":"nginx:latest","State":"exited","Status":"Exited (0) 2 hours ago","Ports":""}
{"ID":"bbb","Names":"db","Image":"postgres:16","State":"running","Status":"Up 3 minutes","Ports":"0.0.0.0:5432->5432/tcp"}
"#;
        let containers = parse_containers(output);
        assert_eq!(containers.len(), 2);
        assert_eq!(containers[0].name, "db");
        assert!(containers[0].running());
        assert_eq!(containers[1].name, "web");
        assert!(!containers[1].running());
    }

    #[test]
    fn parses_image_json() {
        let output = r#"{"ID":"sha256:abc","Repository":"alpine","Tag":"3.20","Size":"7.8MB"}"#;
        let images = parse_images(output);
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].reference(), "alpine:3.20");
        assert_eq!(images[0].size, "7.8MB");
    }

    #[test]
    fn parses_volume_and_network_json() {
        let volumes = parse_volumes(
            r#"{"Driver":"local","Name":"app-data","Mountpoint":"/var/lib/docker/volumes/app-data/_data"}"#,
        );
        assert_eq!(volumes.len(), 1);
        assert_eq!(volumes[0].name, "app-data");
        assert_eq!(volumes[0].driver, "local");

        let networks =
            parse_networks(r#"{"ID":"abc","Name":"app-net","Driver":"bridge","Scope":"local"}"#);
        assert_eq!(networks.len(), 1);
        assert_eq!(networks[0].name, "app-net");
        assert_eq!(networks[0].driver, "bridge");
    }
}
