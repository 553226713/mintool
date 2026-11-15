# 数据库模块规划

当前开发版已落地基础数据库工作台：**SQLite、PostgreSQL、MySQL、MariaDB**。数据库模块是项目、SSH、Docker 之后的第四个主模块，目标是让本地项目、远程环境和容器环境中的数据查询留在同一个工作台内，而不是把 Mintool 做成完整的数据库 IDE。

## 为什么不是本机服务

本机 systemd 服务管理对 Linux 运维有价值，但对当前以项目开发、SSH 和 Docker 为主的用户频率偏低；`systemctl` 与 `journalctl` 已经足够直接，模块很难形成明显增益。数据库则能连接当前已有的开发闭环：

- 打开项目后直接查询项目使用的 SQLite 文件。
- 后续将数据库连接绑定到 SSH 会话，调试测试环境。
- 后续连接 Docker 中常见 PostgreSQL/MySQL 实例。
- 发现数据问题后立即回到项目修改代码。

## 已实现范围

- 活动栏新增“数据库”模块。
- SQLite、PostgreSQL、MySQL 和 MariaDB 连接注册表。
- SQLite 路径支持 `~/` 展开；可通过文件浏览器选择已有数据库，或浏览目录并填写文件名创建新数据库。
- 网络数据库支持主机、端口、数据库、用户名、密码和连接测试。
- 新建/编辑连接时可选择数据库分组。
- 连接支持分组、折叠、拖拽归组、右键移动、上下排序、重命名和删除。
- 连接可展开为“数据库 → Schema → 表/视图 → 字段”，字段显示类型、主键、空值约束和默认值。
- 双击表会在新查询面板生成预览 SQL。
- 每个连接拥有独立的 Dock 工作区和持久化查询文档。
- SQL 代码编辑器与语法高亮，`Ctrl+Enter` 执行光标所在语句。
- 查询在后台线程执行，不阻塞 UI；执行期间可取消。
- 结果表格显示列名、值、NULL、耗时、影响行数和截断状态。
- 单次最多保留 1000 行，避免大结果无限占用内存。
- SQL 文本随工作区保存；结果和正在执行的查询只保存在内存。
- 侧栏搜索、添加和移除连接；移除连接不会删除数据库文件。

连接配置位于（权限 `0600`，网络数据库密码可能包含在配置中）：

```text
~/.mintool/modules/database/registry.json
```

查询工作区位于：

```text
~/.mintool/modules/database/sessions/<连接 UUID>.json
```

## 架构

```text
src/model/query.rs                 # 与模块无关的查询文档和语句定位
src/workspace/layout/state.rs      # 查询文档按 Pane 持久化
src/modules/database/
├── mod.rs                         # 模块运行态和查询视图注册
├── model/                         # 连接、注册表、结构树和结果类型
├── driver.rs                      # 按数据库类型分发查询、测试和结构读取
├── sqlite.rs                      # rusqlite 驱动、结构读取和取消句柄
├── postgres_driver.rs             # PostgreSQL 查询与结构读取
├── mysql_driver.rs                # MySQL/MariaDB 查询与结构读取
├── connection_editor.rs           # 新建/编辑连接表单
├── query_view.rs                  # SQL 编辑器、执行状态、结果表格
├── sidebar.rs                     # 分组连接列表和数据库结构树
├── dialogs.rs                     # 连接、分组和移除确认
└── actions.rs                     # 打开、添加和移除会话
```

工作区只认识 `SessionSurface::Query` 和中立的查询文档。数据库连接、SQLite 驱动和结果表格都由数据库模块拥有，`workspace` 不依赖 `modules::database`。

## 后续阶段

### Phase 2：TLS 与凭据

- PostgreSQL 与 MySQL TLS/SSL 模式配置。
- 密码只保存到系统密钥环。
- SSH 隧道与连接复用。

### Phase 3：查询效率

- 查询历史与收藏。
- 执行选区。
- 结果列宽、复制单元格/行、CSV/JSON 导出。
- 虚拟化结果列表，提高大结果滚动性能。
- 危险语句确认。

### Phase 4：环境联动

- 数据库连接复用 SSH 模块的隧道配置。
- 从 Docker 容器发现数据库端口并创建连接。
- 查询结果中的 JSON 与时间类型专用展示。

Redis 不进入 SQL 工作台。它更适合独立的键值浏览界面，后续作为数据源类型单独设计。
