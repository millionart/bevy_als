# bevy_als — agents.md（执行规则）

用 Bevy ECS 复刻 [ALS-Refactored](https://github.com/Sixze/ALS-Refactored)（v4.18，UE 5.8）的高级运动系统：
以 `bevy_animation_graph`（替代 UE AnimBP）+ `bevy-tnua`（替代 UE CharacterMovement/KCC）为基座，
按需扩展这两个插件，用 Bevy/ECS 思路重建，不背负 UE 技术债。暂不包含网络同步。

**执行内容一律以 [`CHECKLIST.md`](CHECKLIST.md) 为准**（模块清单 + 各模块 Gate）；本文件只规定执行纪律。

## 文档索引

| 文件 | 内容 |
|---|---|
| `agents.md` | 执行规则（本文件） |
| `CHECKLIST.md` | 执行清单：基础事实、M0–M11 模块、每模块 Gate、附录 |
| `goal.md` | goal 执行入口说明 |
| `docs/env.md` | 本地隐私环境（gitignore）：路径、引擎版本、Unreal MCP 连接步骤 |

## 执行规则

1. **自上而下**：严格按 `CHECKLIST.md` 模块顺序执行，不做跨模块跳跃；模块内按清单自上而下推进，其中无数据依赖的清单项可按规则 6 并行。
2. **每项一提交**：实现一个清单项 → 自测通过 → 在 `CHECKLIST.md` 勾选该项 → 连同勾选一次提交（`M<编号>: <条目摘要>`），保持工作区无未提交残留。
3. **Gate 约束**：每个模块最后一个清单项是本模块验收 Gate。Gate 验证必须留可复查证据（命令输出摘要/截图路径，写入该次提交说明）；Gate 未通过不得开始下一模块；后续发现前序模块缺陷时回退修补并重新过 Gate。
4. **插件扩展边界**：基座插件的能力扩展优先经其公开扩展点在本仓库 `crates/als-ext` 实现（如 curve、ControlRig 等价 IK），不修改 fork；仅当公开扩展点确实不足时才修改 fork，且 fork 改动必须独立小步提交、可回馈上游。任何时候禁止 vendor 拷贝插件源码。
5. **资产占位**：动画/模型/曲线资产通过 Unreal MCP 从 ALS-Refactored 工程导出（配置步骤见 `docs/env.md`），在 M0.5 一次性批量完成，此后不得零散补导——确需补充须先在 `CHECKLIST.md` 增补明确条目再执行。导出物放 `assets/` 并登记 `assets/docs/export-manifest.md`；禁止把 ALS 商店原始 uasset 提交进仓库。
6. **并行开发（按需优先）**：同一模块内无数据依赖的清单项，优先拆分给多个 subagents 并行执行（如资产导出与代码实现、相互独立的系统/测试）；不同 subagent 不得修改同一文件，存在写入交集时退回串行。跨模块仍受规则 1 约束不得并行。每个 subagent 只负责单一明确产出，代码合并、提交、勾选与 Gate 验证由主代理统一串行收口。仅当存在多个可并行的独立任务时才派发 subagents；单任务一律由主线程独立完成，不为串行工作调用 subagent。
7. **隐私红线（严格执行）**：仓库内任何被提交的文件（脚本、配置、文档）一律禁止硬编码本地绝对路径、引擎安装位置、本机工程位置、账号等隐私内容。此类值只能通过以下途径注入：gitignored 本地配置（如 `tools/env.local.json`，模板见 `tools/env.example.json`）、环境变量、命令行参数；依赖仓库用公开 git 地址而非 path 依赖。提交前必须自检：`git grep -n -E "D:[/\\\\]|C:[/\\\\]Program Files|Users/"` 无命中（注释性占位符 `<...>` 除外）。违反时立即回退修补。
8. **代码语言纪律（严格执行）**：一切可执行内容（Rust/Python/Shell/PowerShell 源码及其中字符串字面量、日志与断言消息、RON/JSON 资产中的语义字段）只允许英文；同时禁止写注释（含 Rust doc comment 与各语言 # / // 注释；shebang 与格式声明除外）。ALS 对照依据、来源标注等原本要写进注释的信息，一律移入提交说明或 CHECKLIST/Gate 记录。文档类 Markdown（本文件、CHECKLIST.md 等）不受此限。
9. **资源限流（严格执行）**：运行 `cargo build/test/check/clippy` 等重编译任务前，必须确认编译并行度已按当前机器规格限制——依据逻辑核数与可用内存选取 jobs ≈ min(逻辑核数/4, 可用内存GB/3)，钳制到 [2,8]，写入用户级 Cargo 配置（`~/.cargo/config.toml` 的 `[build] jobs`，机器本地、不入仓库）或以 `--jobs N` / 环境变量注入；禁止在未限流的机器上依赖 cargo 默认全核并行跑大型 workspace（多进程 rustc/link 同时满载会耗尽内存导致系统卡死）。长测试运行期间留意系统响应；因未限流造成系统假死视同执行事故，须先修复限流再继续。
10. **禁止**：引入 UE 式继承树/反射式蓝图思维；在无 Gate 记录时宣称模块完成。

## 工具调用教训（M0 实测，防再犯）

1. **UE MCP：toolset 工具必须经 `call_tool` 元工具分发**。协议层 `tools/call` 只注册 `list_toolsets` / `describe_toolset` / `call_tool` 三个元工具；把 toolset 方法名直接当 `name` 调用必报 `Unknown tool`。正确姿势：`name="call_tool"`，并在其 `arguments` 内传 `toolset_name`、`tool_name`、`arguments`（依据引擎源码 `ModelContextProtocolServer.cpp` 的 FindTool 查找逻辑）。统一走仓库封装 `tools/mcp_probe.py`，不要绕过它手写 HTTP 请求。
2. **shell 长命令可能间歇性整块丢参**（报 `command is required`，参数为空）。含大量引号/JSON 转义的长命令风险最高。对策：复杂逻辑先写入脚本文件（如 `tools/*.sh` 或 Python 脚本）再用短命令执行；同一失败调用不得原样反复重试——一次失败即改道脚本化。
