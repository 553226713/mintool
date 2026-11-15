
1:一个类一个文件，但文件不要超过800行
2：按照面向对象思维设计架构
3：注意代码模块边界，不要随意穿透代码层次
4：不要随意把所有代码文件放在一个目录下面，有需要就用目录划分层次
5：软件设计是多功能模块，不同大模块在不同目录下面区分，公共组件在工具地方。善用设计模式设计代码，尽量不要写重复代码要求扩展性高
6：依赖方向固定：services 不得依赖 modules/ui/workspace/shell；workspace 只依赖 model 和中性类型；ui 只放通用组件，模块专属 UI、对话框、工厂放在 modules/<模块>/ 下
7：模块分发集中在 modules/mod.rs 的 Modules（会话解析、选择、保存、远程预览等），不要在 app/workspace 散落 ModuleId 分支；渲染入口的少量 match 只允许出现在组合根
8：对话框统一用 ui/dialogs 的通用壳 + DialogContent，业务逻辑写在 modules/<模块>/dialogs.rs，禁止在 ui 层写模块业务
9：同一逻辑只写一次：路径、大小格式化、分组标题、资源行等公共代码放 ui/common 或 modules/common，发现两处相同实现必须合并
10：目录约定：ui/{common,dialogs,preview,terminal}，modules/<模块>/{model,dialogs,...}，model/session 存放中立会话类型；新增文件沿用现有层级，不要平铺
11：每次改动至少通过 cargo check --tests，并保持现有测试全绿；不要用堆大文件或多个类型挤一个文件来图快
