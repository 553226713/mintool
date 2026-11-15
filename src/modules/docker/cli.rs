use super::model::{
    ComposeProject, ComposeService, DockerContainer, DockerImage, DockerNetwork, DockerVolume,
};
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
    // The four queries are independent; run them concurrently so a refresh
    // costs roughly one Docker CLI round-trip instead of four.
    std::thread::scope(|scope| {
        let containers = scope.spawn(list_containers);
        let images = scope.spawn(list_images);
        let volumes = scope.spawn(list_volumes);
        let networks = scope.spawn(list_networks);
        Ok((
            join_thread(containers)?,
            join_thread(images)?,
            join_thread(volumes)?,
            join_thread(networks)?,
        ))
    })
}

fn join_thread<'a, T>(result: std::thread::ScopedJoinHandle<'a, Result<T>>) -> Result<T> {
    result
        .join()
        .map_err(|_| anyhow::anyhow!("Docker 查询线程异常退出"))?
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

pub fn prune(resource: super::model::DockerResource) -> Result<String> {
    let args: &[&str] = match resource {
        super::model::DockerResource::Containers => &["container", "prune", "-f"],
        super::model::DockerResource::Images => &["image", "prune", "-f"],
        super::model::DockerResource::Volumes => &["volume", "prune", "-f"],
        super::model::DockerResource::Networks => &["network", "prune", "-f"],
        super::model::DockerResource::Compose => {
            bail!("Compose 项目不支持清理")
        }
    };
    run(args, "60").map(|output| output.trim().to_string())
}

pub fn pull_image(reference: &str) -> Result<()> {
    run(&["pull", reference], "300").map(|_| ())
}

pub fn tag_image(id: &str, reference: &str) -> Result<()> {
    run(&["tag", id, reference], "20").map(|_| ())
}

pub fn create_volume(name: &str) -> Result<()> {
    run(&["volume", "create", name], "20").map(|_| ())
}

pub fn create_network(name: &str) -> Result<()> {
    run(&["network", "create", name], "20").map(|_| ())
}

pub fn compose_projects() -> Result<Vec<ComposeProject>> {
    let output = run(&["compose", "ls", "--all", "--format", "json"], "30")?;
    let mut projects = parse_compose_projects(&output);
    for project in &mut projects {
        if let Ok(output) = run(
            &[
                "compose",
                "-p",
                project.name.as_str(),
                "ps",
                "--all",
                "--format",
                "json",
            ],
            "30",
        ) {
            project.services = parse_compose_services(&output);
        }
    }
    Ok(projects)
}

pub fn compose_action(project: &str, action: &str) -> Result<()> {
    let mut args = vec!["compose".to_string(), "-p".to_string(), project.to_string()];
    match action {
        "up" => {
            args.push("up".to_string());
            args.push("-d".to_string());
        }
        "down" => args.push("down".to_string()),
        "restart" => args.push("restart".to_string()),
        _ => bail!("不支持的 Compose 操作"),
    }
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    run(&refs, "180").map(|_| ())
}

fn parse_compose_projects(output: &str) -> Vec<ComposeProject> {
    let Ok(value) = serde_json::from_str::<Value>(output.trim()) else {
        return Vec::new();
    };
    let Some(items) = value.as_array() else {
        return Vec::new();
    };
    let mut projects: Vec<_> = items
        .iter()
        .filter_map(|item| {
            let name = item
                .get("Name")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .to_string();
            if name.is_empty() {
                return None;
            }
            Some(ComposeProject {
                name,
                status: item
                    .get("Status")
                    .and_then(|value| value.as_str())
                    .unwrap_or("")
                    .to_string(),
                config_files: item
                    .get("ConfigFiles")
                    .and_then(|value| value.as_str())
                    .unwrap_or("")
                    .to_string(),
                services: Vec::new(),
            })
        })
        .collect();
    projects.sort_by(|left, right| left.name.cmp(&right.name));
    projects
}

fn parse_compose_services(output: &str) -> Vec<ComposeService> {
    let Ok(value) = serde_json::from_str::<Value>(output.trim()) else {
        return Vec::new();
    };
    let Some(items) = value.as_array() else {
        return Vec::new();
    };
    let text = |item: &Value, key: &str| {
        item.get(key)
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .to_string()
    };
    let mut services: Vec<_> = items
        .iter()
        .map(|item| ComposeService {
            name: text(item, "Name"),
            service: text(item, "Service"),
            state: text(item, "State"),
            ports: text(item, "Ports"),
        })
        .collect();
    services.sort_by(|left, right| left.service.cmp(&right.service));
    services
}

/// One-shot `docker stats` snapshot formatted for the details dialog.
pub fn stats_summary() -> Result<String> {
    let output = run(&["stats", "--no-stream", "--format", "{{json .}}"], "20")?;
    let mut lines = vec![format!(
        "{:<24} {:>8} {:>18} {:>14} {:>14} {:>6}",
        "名称", "CPU", "内存", "网络 I/O", "磁盘 I/O", "PIDs"
    )];
    for line in output.lines() {
        let Ok(value) = serde_json::from_str::<Value>(line.trim()) else {
            continue;
        };
        let text = |key: &str| {
            value
                .get(key)
                .and_then(|value| value.as_str())
                .unwrap_or("-")
                .to_string()
        };
        lines.push(format!(
            "{:<24} {:>8} {:>18} {:>14} {:>14} {:>6}",
            text("Name"),
            text("CPUPerc"),
            text("MemUsage"),
            text("NetIO"),
            text("BlockIO"),
            text("PIDs")
        ));
    }
    if lines.len() == 1 {
        lines.push("没有运行中的容器".to_string());
    }
    Ok(lines.join("\n"))
}

pub fn logs_spec(id: &str) -> Vec<String> {
    logs_spec_with(id, 500, false)
}

pub fn logs_spec_with(id: &str, tail: u32, timestamps: bool) -> Vec<String> {
    let mut args = vec![
        "logs".to_string(),
        "-f".to_string(),
        "--tail".to_string(),
        tail.to_string(),
    ];
    if timestamps {
        args.push("--timestamps".to_string());
    }
    args.push(id.to_string());
    args
}

pub fn run_image(
    image: &str,
    name: &str,
    ports: &str,
    env: &str,
    mounts: &str,
    command: &str,
) -> Result<String> {
    let mut args = vec!["run".to_string(), "-d".to_string()];
    if !name.trim().is_empty() {
        args.push("--name".to_string());
        args.push(name.trim().to_string());
    }
    for port in ports
        .split([',', ' '])
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        args.push("-p".to_string());
        args.push(port.to_string());
    }
    for variable in env.split_whitespace().filter(|part| part.contains('=')) {
        args.push("-e".to_string());
        args.push(variable.to_string());
    }
    for mount in mounts
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        args.push("-v".to_string());
        args.push(mount.to_string());
    }
    args.push(image.to_string());
    args.extend(command.split_whitespace().map(str::to_string));
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    Ok(run(&refs, "60")?.trim().to_string())
}

pub fn inspect_container(id: &str) -> Result<String> {
    let output = run(&["inspect", "--format", "{{json .}}", id], "10")?;
    let value: Value = serde_json::from_str(output.trim()).context("无法解析容器详情")?;
    let mut lines = Vec::new();
    let id = json_str(&value, &["Id"]);
    let name = json_str(&value, &["Name"]);
    lines.push(format!(
        "ID        {}",
        if id.is_empty() { "-" } else { &id }
    ));
    lines.push(format!(
        "名称      {}",
        name.trim_start_matches('/').to_string()
    ));
    lines.push(format!(
        "镜像      {}",
        json_str(&value, &["Config", "Image"])
    ));
    lines.push(format!(
        "状态      {}",
        json_str(&value, &["State", "Status"])
    ));
    lines.push(format!(
        "启动      {}",
        json_str(&value, &["State", "StartedAt"])
    ));
    lines.push(format!("创建      {}", json_str(&value, &["Created"])));
    let mut command = json_strings(&value, &["Config", "Entrypoint"]);
    command.extend(json_strings(&value, &["Config", "Cmd"]));
    if !command.is_empty() {
        lines.push(format!("命令      {}", command.join(" ")));
    }
    let ports = json_ports(&value);
    if !ports.is_empty() {
        lines.push("端口".to_string());
        lines.extend(ports.into_iter().map(|port| format!("  {port}")));
    }
    let env = json_strings(&value, &["Config", "Env"]);
    if !env.is_empty() {
        lines.push("环境变量".to_string());
        lines.extend(env.into_iter().map(|item| format!("  {item}")));
    }
    if let Some(mounts) = value.get("Mounts").and_then(|mounts| mounts.as_array()) {
        if !mounts.is_empty() {
            lines.push("挂载".to_string());
            for mount in mounts {
                let source = mount.get("Source").and_then(|v| v.as_str()).unwrap_or("-");
                let destination = mount
                    .get("Destination")
                    .and_then(|v| v.as_str())
                    .unwrap_or("-");
                let mode = mount.get("Mode").and_then(|v| v.as_str()).unwrap_or("");
                lines.push(format!("  {source} → {destination} {mode}"));
            }
        }
    }
    let networks = value
        .pointer("/NetworkSettings/Networks")
        .and_then(|networks| networks.as_object())
        .map(|networks| networks.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    if !networks.is_empty() {
        lines.push("网络".to_string());
        lines.extend(networks.into_iter().map(|name| format!("  {name}")));
    }
    Ok(lines.join("\n"))
}

fn json_str(value: &Value, path: &[&str]) -> String {
    path.iter()
        .try_fold(value, |current, key| current.get(*key))
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string()
}

fn json_strings(value: &Value, path: &[&str]) -> Vec<String> {
    path.iter()
        .try_fold(value, |current, key| current.get(*key))
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn json_ports(value: &Value) -> Vec<String> {
    let Some(ports) = value
        .pointer("/NetworkSettings/Ports")
        .and_then(|ports| ports.as_object())
    else {
        return Vec::new();
    };
    let mut result = Vec::new();
    for (container, binds) in ports {
        let Some(binds) = binds.as_array().filter(|binds| !binds.is_empty()) else {
            result.push(container.clone());
            continue;
        };
        for bind in binds {
            let host_ip = bind.get("HostIp").and_then(|v| v.as_str()).unwrap_or("");
            let host_port = bind.get("HostPort").and_then(|v| v.as_str()).unwrap_or("");
            result.push(format!("{host_ip}:{host_port} → {container}"));
        }
    }
    result.sort();
    result
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
