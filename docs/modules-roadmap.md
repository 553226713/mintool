# 模块 roadmap 与 Docker 模块设计

当前模块：**项目、SSH、Docker、数据库**。Docker 已实现容器、镜像、卷、网络、Compose、运行容器、日志/终端会话和常用资源操作；数据库基础版已支持 SQLite、PostgreSQL、MySQL、MariaDB，以及连接分组、结构树和查询工作区，详见 [database-module.md](database-module.md)。

## 一、为什么是 Docker

- 现有能力可直接复用：PTY、终端、Dock、分屏、`SessionKey`、模块侧栏、活动栏、状态栏。
- 主要操作（logs、exec、compose logs）本质上都是“在终端里跑一条命令”，与 SSH 会话结构一致。
- 用户当前需要频繁手敲 `docker ps / logs / exec`，列表 + 右键操作收益明显。
- 不需要账号体系或远端凭据，先做本地 Docker Engine，风险可控。

## 二、Docker 模块目标交互

```text
┌────┬──────────────────┬────────────────────────────────┬──────────────┐
│ 📁 │ 容器              │ 工作空间 › Docker › web       │ 容器详情      │
│ 🖥 │ 搜索容器…         │ ┌────────────────────────────┐ │ 镜像/端口     │
│ 🐳 │ ● web   running  │ │ 终端 / 日志 / 分屏          │ │ 环境/挂载     │
│    │ ○ db    exited   │ └────────────────────────────┘ │ 操作按钮      │
│ ⚙ │ 镜像              │                                │              │
└────┴──────────────────┴────────────────────────────────┴──────────────┘
```

- 活动栏新增 Docker 图标，与项目/SSH 并列。
- 侧栏是 **容器 / 镜像 / 卷 / 网络** 资源分类；右侧展示当前分类并直接提供管理操作。
- 双击容器（或右键“打开”）创建该容器的会话工作区：
  - 默认打开日志标签：`docker logs -f --tail 500 <id>`。
  - 右键“进入终端”新建 exec 终端：优先 `/bin/sh`，失败再试 `/bin/bash`。
  - 同一容器可开多个标签/分屏（复用 Dock 与会话布局）。
- 容器操作：启动、停止、重启、暂停、删除（二次确认）、复制 ID/名称、查看详情。
- 镜像操作：查看列表、删除（确认）、从镜像运行容器（表单：名称、端口映射、环境变量、挂载、命令）。
- 右侧“容器详情”面板：状态、镜像、启动时间、端口、环境变量、挂载、网络；默认不显示大段 JSON，提供“查看 inspect”按需展开。
- 状态栏：Docker 版本 / 运行中容器数 / 当前容器状态。
- 首次使用无权限时，明确提示 Docker socket 权限或 rootless 配置，而不是静默失败。

## 三、实现方式

第一版使用 **系统 docker CLI + 手动刷新**，不直接实现 Docker API：

- 与 SSH 模块的策略一致：复用系统工具，天然支持 `DOCKER_HOST`、rootless、Podman（通过别名/兼容命令）。
- 列表刷新：
  - 由刷新按钮显式执行 Docker CLI 查询，不做后台轮询。
  - 容器、镜像、卷、网络输出解析为结构化数据；解析函数可单测。
- 会话命令：
  - 日志：`docker logs -f --tail 500 <id>`。
  - Exec：`docker exec -it <id> /bin/sh`（失败后提示改用自定义命令）。
  - 都通过现有 `TerminalSpec { program, args, cwd, envs }` 启动，不新增终端实现。
- 后续如果 CLI 轮询成为瓶颈，再引入 Docker Engine HTTP API（Unix socket），数据模型和 UI 不需要重写。

## 四、目录与数据结构

```text
src/modules/docker/
├── mod.rs          # DockerModule：列表、选中容器、过滤器、刷新时间
├── model.rs        # ContainerSummary / ImageSummary / ContainerDetail
├── cli.rs          # docker 命令执行与输出解析，可单测
├── sidebar.rs      # 容器/镜像列表、右键菜单
├── session.rs      # 容器会话：logs/exec 的 TerminalSpec、打开/关闭
├── explorer.rs     # 右侧容器详情
├── dialogs.rs      # 运行镜像、删除确认等
└── menus.rs        # 容器/镜像右键操作
```

运行态：

```rust
pub struct DockerModule {
    pub containers: Vec<ContainerSummary>,
    pub images: Vec<ImageSummary>,
    pub selected_container: Option<String>,
    pub opened_container: Option<String>,
    pub filter: Entity<InputState>,
    pub expanded: bool,
    pub last_refresh: Instant,
    pub error: Option<String>,
}
```

会话：

- `SessionKey::docker(container_id)`。
- `WorkspaceState` 复用；终端标签由 docker 命令启动。
- 会话布局保存在 `~/.mintool/modules/docker/sessions/<container-id>.json`。
- Docker 模块不持久化连接凭据，只保存 UI 状态（筛选、最近打开的容器）和会话布局。

## 五、实施阶段

**Phase D1：只读与终端（已实现）**
- Docker 图标 / 侧栏 / 容器与镜像列表 / 刷新。
- 双击容器打开日志标签；右键进入 exec 终端。
- 单元测试：`ps` / `images` JSON 解析、状态映射。

**Phase D2：常用操作（已实现）**
- 启动、停止、重启、暂停/恢复、删除（确认）。
- 复制 ID/名称。
- 权限错误和 daemon 未启动的明确提示。

**Phase D3：运行与镜像（已实现）**
- 从镜像运行容器（名称、端口、环境变量、挂载、命令）。
- 镜像删除；容器日志跟随/暂停；一键清理已停止容器。

**Phase D4：Compose 与高级项（已实现）**
- Compose 项目列表、up/down/restart、compose logs。
- 卷、网络清理能力已实现；Compose 资源与批量清理待补充。
- 容器指标（可选，cpu/mem 需要 `docker stats` 解析）。

## 六、集成点

新增 Docker 模块只需要：

1. `ModuleId` 增加 `Docker`，`MODULES` 增加描述。
2. `Modules` 增加 `docker: DockerModule`。
3. 四处静态分发补分支：侧栏、会话、状态栏、对话框。
4. `SessionKey::docker(id)` 接入现有 workspace/dock。
5. 存储命名空间自动使用 `~/.mintool/modules/docker/`。

## 七、测试方式

本机已有 Docker，可直接跑两个测试容器：

```bash
docker run -d --name mintool-docker-a alpine sleep 3600
docker run --rm alpine echo hello
```

验证：

- 运行中/已退出容器状态与排序
- 日志标签与断开重连
- exec 进入 alpine 的 `/bin/sh`
- 停止/启动/删除后列表刷新
- daemon 停止或权限不足时的错误提示

CI 环境下没有 Docker 时跳过集成测试，只保留 CLI 输出解析单测。

## 八、模块路线建议

1. **数据库（基础版完成）**：下一步是查询历史、结果导出、SSH 隧道和更多驱动。
2. **快捷命令 / Runbook**：保存常用命令并在项目/SSH/Docker 会话中执行，建议做成跨模块能力而不是独立资源模块。
3. **本机服务**：仅当产品明确转向 Linux 运维用户时再做 systemd 服务与端口监听。
4. **Redis 数据源**：需要独立的键值浏览交互，不并入 SQL 工作台。

当前活动栏有四个主模块：项目、SSH、Docker、数据库。后续模块继续沿用同一套 `ModuleId + SessionKey + 模块目录` 机制；非终端内容通过 `SessionSurface` 扩展，不需要让 workspace 依赖具体业务模块。
