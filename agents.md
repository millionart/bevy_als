# bevy_als — agents.md（执行规则）

用 Bevy ECS 复刻 [ALS-Refactored](https://github.com/Sixze/ALS-Refactored)（v4.18，UE 5.8）的高级运动系统：
以 `bevy_animation_graph`（替代 UE AnimBP）+ `bevy-tnua`（替代 UE CharacterMovement/KCC）为基座，
按需扩展这两个插件，用 Bevy/ECS 思路重建，不背负 UE 技术债。暂不包含网络同步。

**执行内容一律以 [`CHECKLIST.md`](CHECKLIST.md) 为准**（模块清单 + 各模块 Gate）；本文件只规定执行纪律。

## 文档索引

| 文件 | 内容 |
|---|---|
| `agents.md` | 执行规则（本文件） |
| `CHECKLIST.md` | 当前执行项与持续基础事实；完成批次见归档索引 |
| `goal.md` | goal 执行入口说明 |
| `docs/env.md` | 本地隐私环境（gitignore）：路径、引擎版本、Unreal MCP 连接步骤 |

## 执行规则

1. **自上而下**：严格按 `CHECKLIST.md` 模块顺序执行，不做跨模块跳跃；模块内按清单自上而下推进，其中无数据依赖的清单项可按规则 6 并行。
2. **每项一提交**：实现一个清单项 → 自测通过 → 在 `CHECKLIST.md` 勾选该项 → 连同勾选一次提交（`M<编号>: <条目摘要>`），保持工作区无未提交残留。
3. **Gate 约束**：每个模块最后一个清单项是本模块验收 Gate。Gate 验证必须留可复查证据（命令输出摘要/截图路径，写入该次提交说明）；Gate 未通过不得开始下一模块；后续发现前序模块缺陷时回退修补并重新过 Gate。
4. **依赖修复与插件扩展边界**：任何依赖缺陷或能力缺口一律优先在本仓库内解决，先使用公开扩展点，并在 `crates/als-ext`、适配器或项目侧调度中实现（如 curve、ControlRig 等价 IK、底座 guard）；禁止默认修改、提交或推送依赖项目/fork。仅当项目内方案经实证确实无法满足验收时，允许先自行修改依赖 fork，并在报告中记录技术阻塞、替代方案与维护影响；fork 改动必须独立小步提交、可回馈上游。任何时候禁止 vendor 拷贝插件源码。
5. **资产占位**：动画/模型/曲线资产通过 Unreal MCP 从 ALS-Refactored 工程导出（配置步骤见 `docs/env.md`），在 M0.5 一次性批量完成，此后不得零散补导——确需补充须先在 `CHECKLIST.md` 增补明确条目再执行。导出物放 `assets/` 并登记 `assets/docs/export-manifest.md`；禁止把 ALS 商店原始 uasset 提交进仓库。
6. **并行开发（按需优先）**：同一模块内无数据依赖的清单项，优先拆分给多个 subagents 并行执行（如资产导出与代码实现、相互独立的系统/测试）；不同 subagent 不得修改同一文件，存在写入交集时退回串行。跨模块仍受规则 1 约束不得并行。每个 subagent 只负责单一明确产出，代码合并、提交、勾选与 Gate 验证由主代理统一串行收口。仅当存在多个可并行的独立任务时才派发 subagents；单任务一律由主线程独立完成，不为串行工作调用 subagent。
7. **隐私红线（严格执行）**：仓库内任何被提交的文件（脚本、配置、文档）一律禁止硬编码本地绝对路径、引擎安装位置、本机工程位置、账号等隐私内容。此类值只能通过以下途径注入：gitignored 本地配置（如 `tools/env.local.json`，模板见 `tools/env.example.json`）、环境变量、命令行参数；依赖仓库用公开 git 地址而非 path 依赖。提交前必须自检：`git grep -n -E "D:[/\\\\]|C:[/\\\\]Program Files|User[s]/"` 无命中（注释性占位符 `<...>` 除外）。违反时立即回退修补。
8. **代码语言纪律（严格执行）**：一切可执行内容（Rust/Python/Shell/PowerShell 源码及其中字符串字面量、日志与断言消息、RON/JSON 资产中的语义字段）只允许英文；同时禁止写注释（含 Rust doc comment 与各语言 # / // 注释；shebang 与格式声明除外）。ALS 对照依据、来源标注等原本要写进注释的信息，一律移入提交说明或 CHECKLIST/Gate 记录。文档类 Markdown（本文件、CHECKLIST.md 等）不受此限。
9. **资源限流（严格执行）**：运行 `cargo build/test/check/clippy` 等重编译任务前，必须确认编译并行度已按当前机器规格限制——依据逻辑核数与可用内存选取 jobs ≈ min(逻辑核数/4, 可用内存GB/3)，钳制到 [2,8]，写入用户级 Cargo 配置（`~/.cargo/config.toml` 的 `[build] jobs`，机器本地、不入仓库）或以 `--jobs N` / 环境变量注入；禁止在未限流的机器上依赖 cargo 默认全核并行跑大型 workspace（多进程 rustc/link 同时满载会耗尽内存导致系统卡死）。长测试运行期间留意系统响应；因未限流造成系统假死视同执行事故，须先修复限流再继续。
10. **多 worktree 构建隔离（严格执行）**：每个 worktree 一律使用自身 `target/`，禁止以 `CARGO_TARGET_DIR` 指向其他 checkout 的 target 目录（跨树共享缓存 = 共享目录锁：cargo 对 target 整目录独占加锁，任一侧构建期间另一侧所有 cargo 命令排队数分钟，`--examples` 级全量构建可达十几分钟）。**创建 worktree 必须用 `pwsh tools\new-worktree.ps1 -Name <名> [-Branch <分支>]`**——脚本自动完成 worktree add + 追踪文件 mtime 同步 + 构建缓存拷贝（新 worktree 首建 16 分钟→实测 7.8 秒）；禁止裸 `git worktree add`（新树无 target，裸建要么 16 分钟全量重编、要么诱发跨树共享指入）。调试/试玩/门禁一律在自身 worktree 的 target 内进行；根仓库 `run_manual.bat` 已固定隔离目录 `target_game` 即为此设计，勿改回共享。遇到 "Blocking waiting for file lock on artifact directory" 先排查跨 worktree 指入与残留构建进程，勿盲目等待。
11. **禁止**：引入 UE 式继承树/反射式蓝图思维；在无 Gate 记录时宣称模块完成。
12. **实时对标与实证验收（严格执行）**：任何对标 ALS-Refactored 的行为实现/修改，动手前必须从 ALS-Refactored 工程实时提取当次相关的事实（uasset 二进制解析 / Unreal MCP / 插件 C++ 源码，路径见 gitignored `docs/env.md`）；`docs/env.md` 指定的本地源码或工程可用时必须优先以其为准，禁止用网页／GitHub 版本替代或覆盖本地事实。仅当该本地路径不可用或确实不含所需事实时，才可使用公开源码作为后备，并在 CHECKLIST 条目或提交说明记录原因。禁止凭记忆、印象或"经典 ALS 印象"推断行为；提取到的事实（状态机结构/转换条件/参数数值/函数语义）登记进 CHECKLIST 条目或提交说明后方可编码。验收一律以**真实运行日志结论**为标准：gate 断言必须落在探针/pin/数值等可复查证据上，禁止以截图目测、"应当正确"的推断或单帧姿态猜测代替；证据不足时补插桩取证，不得宣布通过。
13. **待执行条目隔离（严格执行）**：创建或补充 `CHECKLIST.md` 条目本身不授权调研、源码提取、uasset/MCP 查询、实现或验收。若该条目不在当前依赖顺序中立即执行，必须只登记其目标、依赖和验收条件；待其成为当前执行项后，才可从 ALS-Refactored 实时提取该项事实并开始工作。此前为待执行项取得的事实不得作为实施依据，开始执行时必须重新提取。
14. **清单归档（严格执行）**：只可归档**明确命名且已完成／已废止的清单段落**，以其 Markdown 起止标题为精确边界；不得因为根清单暂时没有未勾选项就归档整个 `CHECKLIST.md`。基础事实、模块依赖顺序、附录、未来任务入口、仍有效的规范性说明以及任何未被该段落标题包围的内容必须保留在根清单。归档前必须在工作说明中列出“将移动的标题”和“将保留的标题”，并在 `git diff` 中核对范围；归档文件只能承载被移动的段落及简短来源说明。`docs/archive/archivedchecklist.md` 必须只作按日期链接的索引，不承载清单正文。归档后同步更新所有路径引用与静态验收读取路径，并单独提交。**创建新的未完成清单模块前，必须先检查根清单中的完整已完成段落；存在时先完成该段归档、更新索引并单独提交，之后才可登记新模块。归档提交不得与新任务登记或实现混合。**
15. **通用能力与项目语义归属（严格执行）**：涉及依赖、物理、动画图或控制器能力缺口时，实施前必须评估通用机制与项目语义的边界。可复用的底层桥接／扩展优先实现于 `crates/als-ext`；ALS 专用规则、源参数映射、资产选择和角色生命周期保留在主项目或 `als-core`。在 `CHECKLIST.md` 条目或提交说明记录归属决定与理由；不得以项目专用常量污染通用扩展，也不得把通用能力留在单一 ALS 调度层。
16. **真实入口集成（严格执行）**：任何已实现的运行时功能、缺陷修复、系统、资产选择和动画图，最终都必须接入 `run_manual.bat` 启动的 `manual_playground` 真实入口，不能只存在于独立 Gate、示例或静态测试中。实现完成前，必须在真实入口添加或更新可重复触发的自动场景、HUD 或 probe，覆盖该功能的实际行为并留下真实运行日志；完整运行时装配及每个功能／图的“已执行”状态必须可复查。仅记录插件注册、组件存在或静态图文件不得作为集成验收。未覆盖项必须明确记录为未覆盖并使入口集成 Gate 不通过，直至补齐实际行为验证；新增功能的 `CHECKLIST.md` 条目必须同时登记其 `manual_playground` 覆盖路径。
17. **真实试玩场景保真（严格执行）**：`manual_playground` 的自动集成验收必须在已加载、可见的原始 Playground 地形、环境和正常角色相机中执行。禁止为求稳定而将角色传送到场外坐标、纯白／空白空间、独立测试世界，或只含 collider 的隐形地板；这类路径不得计入真实入口覆盖。确需加入 Mantle 台、移动基座、相机障碍等辅助物时，只能作为当前 Playground 内、落在其真实地面上的短时可见摆件，必须同时有可渲染 mesh/material、物理 collider、明确日志位置和清理／生命周期；辅助物不能替代 Playground 原有地面、环境碰撞或角色出生／相机链。驱动接管前必须先通过入口既有的场景物理、PlayerStart、角色可见性和 grounded spawn Gate；验收日志与必要截图须证明角色始终处于用户可实际试玩的 Playground 画面，而非脱离入口的隔离环境。

18. **动画图编辑器完整性（严格执行）**：任何创建或修改 `assets/graphs/**/*.animgraph.ron`、`assets/graphs/**/*.fsm.ron` 的工作，在编辑器启动和提交前必须先运行 `python tools/validate_animgraph_editor_assets.py --root assets/graphs --fix`，再运行不带 `--fix` 的同一命令；后者必须以 exit 0 结束。该验收要求 `input_order` 与 `input_times`／`input_data` 精确覆盖且无陈旧项、重复项或非法序号；每个 FSM state 必须有有限且不重叠的 editor 坐标；每条 transition 必须指向已声明 state。`source == target` 的自环是有效语义，禁止为了规避图形问题删除或改写它；含自环的图在编辑器依赖升级或绘制代码改动时，必须运行 `python tools/verify_animgraph_editor_smoke.py --jobs N`；该命令无窗口地执行 `self_transition_arrow_uses_finite_geometry` 回归测试，作为最终证据。人工打开编辑器只用于视觉布局或交互评审，不是默认 Gate。`als_graph_editor.bat` 的只读预检不得移除或绕过；改动三个图资产修复／验收工具本身时，必须同时运行各自的 `--self-test` 和全量只读验收。

19. **跨引擎 Transform 保真（严格执行）**：从 Unreal 向 Bevy 移植时，任何涉及 transform 的数据或行为都必须先按两引擎的完整三维坐标基、单位、手性与旋转约定推导后再转换；禁止直接抄写或只缩放 XYZ 数值。范围包括但不限于 primitive/collider、骨骼与虚骨、socket、角色与相机、层级局部／世界变换、constraint 两端 local frame 与 axis、IK target 与 pole、root motion、动画姿态读写、实例化偏移、trace/query 起终点及调试 probe。位置、方向、extents/scale、Euler、quaternion、matrix 和每个 parent-relative transform 都必须使用同一已验证的基变换；转换函数须有基向量与旋转不变量测试，静态审计须逐字段比对 source/target，真实入口日志须记录可复查的 source→target transform 证据。出现坐标定义、骨骼导入轴或 parent space 不确定时，必须先从本地 UE 源工程/资产实时提取事实并补 probe，未证实前不得编码或宣称等价。

20. **初始化、重建与逐帧执行分相（严格执行）**：移植 Unreal 的任何生命周期链路前，必须从当次本地资产／源码明确标注每个节点或函数属于一次性初始化／Construction、条件性重建／重新初始化、事件驱动，还是每帧 Update／Forwards Solve；禁止因实现方便把 Construction、BeginPlay、首次激活、缓存骨骼／引用姿态、默认变量写入或资源装配放进逐帧系统，也禁止把逐帧状态刷新错误地冻结在初始化阶段。Bevy 实现必须以独立 component／resource 与显式 SystemSet 表示各相位：初始化仅在实体创建或明确定义的 reinitialize 条件下执行一次，逐帧系统只能消费已构造状态；需要重建时必须有可复查的触发条件、先清理旧 transient state 再重新执行一次性链路。验收必须同时记录初始化执行次数、重建次数及连续多帧 runtime stage 顺序，证明 Construction 不会逐帧重复而每帧链路也不会漏跑；静态审计与真实入口 probe 缺任一证据均不得宣称等价。

21. **跨引擎单位与坐标基离线转换（严格执行）**：跨引擎移植的长度、坐标轴、手性、旋转、scale、matrix、parent-relative transform 与任何由其派生的值，必须由仓库内可复跑的离线导入／生成／审计工具完成转换；工具使用中性的 source coordinate basis／runtime coordinate basis 命名，不得以某一引擎名称充当通用 runtime API。转换结果必须物化为提交的项目资产、生成源码常量或明确的 target-space 配置，并由静态审计逐字段复算验证；运行时禁止调用通用 source→target 坐标／单位 bridge，也不得读取源引擎数值后临时换轴或换单位。运行时只可消费已经转换的 target-space 数据，并用 target-space 算法处理动态状态；若确有外部实时流输入的例外，必须在 `CHECKLIST.md` 单列数据边界、转换工具、缓存格式与真实日志证据，未列明不得实现。

22. **动画图作者性 skill 与声明式 Gate（严格执行）**：任何创建或修改 `assets/graphs/**/*.animgraph.ron`、`assets/graphs/**/*.fsm.ron` 的工作，动手前必须完整读取项目内 `skills/animation-graph-equivalence/SKILL.md` 并按规则 12 重新提取当次本地 source fact。每个变更 scope 必须创建或更新 `tools/animgraph_authoring/` 下的 manifest，完整登记实际 RON/FSM 的 node/state 顺序、每个 position、edge/transition、root IO、source-evidence SHA-256、非 Pose root output ABI exception、source/layout/editor 只读检查及 contract/manual Gate；一个 manifest 覆盖多个拆分图时不得遗漏任一被改图。每个非 Pose root output 必须有具体 reason 和 source/runtime evidence；不得把曲线、time、event、控制或物理 ABI 为了减少连线而删除，空 `Inputs` editor decoration 也不得伪造为 asset input。提交前必须执行 `python tools/validate_animgraph_authoring.py --manifest <manifest> --check`，并按规则 18 先运行 editor validator `--fix`、再只读复验、再运行 position verifier；随后必须执行 manifest 声明的 source audit、focused runtime Gate 和 `run_manual.bat` 真实 Playground case。修改本 skill、该 gate 或其 manifest schema 时，必须运行 skill quick validator、gate `--self-test`、全量 graph editor 只读验收及关联 manifest smoke。非图资产仅在不伪造图表示的前提下使用 skill 的 source/target-space/lifecycle/real-entry 纪律。

## 工具调用教训（实测，防再犯）

1. **UE MCP：toolset 工具必须经 `call_tool` 元工具分发**。协议层 `tools/call` 只注册 `list_toolsets` / `describe_toolset` / `call_tool` 三个元工具；把 toolset 方法名直接当 `name` 调用必报 `Unknown tool`。正确姿势：`name="call_tool"`，并在其 `arguments` 内传 `toolset_name`、`tool_name`、`arguments`（依据引擎源码 `ModelContextProtocolServer.cpp` 的 FindTool 查找逻辑）。统一走仓库封装 `tools/mcp_probe.py`，不要绕过它手写 HTTP 请求。
2. **shell 长命令可能间歇性整块丢参**（报 `command is required`，参数为空）。含大量引号/JSON 转义的长命令风险最高。对策：复杂逻辑先写入脚本文件（如 `tools/*.sh` 或 Python 脚本）再用短命令执行；同一失败调用不得原样反复重试——一次失败即改道脚本化。
3. **清构建残留进程前先验归属**。超时/中断的 cargo 会留下孤儿 rustc/link 持锁；但其他 worktree 可能以 `CARGO_TARGET_DIR` 指入本仓库 target（见规则 10）。杀 `cargo/rustc/link.exe` 前必须经 `Win32_Process` 核对该进程命令行与 CWD 归属——曾误杀用户另一 worktree 的合法构建。另：VS Code 的 rust-analyzer 后台 check 也会长时间持锁，属正常现象，用 `rust-analyzer.cargo.targetDir: true`（.vscode/settings.json 已配）隔离而非杀进程。
