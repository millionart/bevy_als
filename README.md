# bevy_als

用 Bevy ECS 复刻 [ALS-Refactored](https://github.com/Sixze/ALS-Refactored)（v4.18，UE 5.8）的高级运动系统。

- **不做代码级移植**：以 [bevy_animation_graph](https://github.com/mbrea-c/bevy_animation_graph)（替代 AnimBP）与 [bevy-tnua](https://github.com/idanarye/bevy-tnua)（替代 CharacterMovement）为基座，按需扩展。
- **不含网络同步**。
- 执行规则见 [agents.md](agents.md)，当前执行清单见 [CHECKLIST.md](CHECKLIST.md)，已完成批次见 [归档索引](docs/archive/archivedchecklist.md)，启动说明见 [goal.md](goal.md)。
- 本地环境配置见 `docs/env.md`（隐私，不入库）；UE 取证统一使用仓库原生 uasset 读取器。

## 基座插件 fork

| upstream | fork | 用途 |
|---|---|---|
| mbrea-c/bevy_animation_graph | [millionart/bevy_animation_graph](https://github.com/millionart/bevy_animation_graph) | 动画图（AnimBP 替代） |
| idanarye/bevy-tnua | [millionart/bevy-tnua](https://github.com/millionart/bevy-tnua) | 浮动角色控制器（CharacterMovement 替代） |

项目使用单一 Cargo 包，已移除历史 `crates` 拆分。源码统一按源归属放在根 `src/`：`animation_graph_ext`、`avian_ext`、`bevy_ext`、`control_rig`、`tnua_ext` 与 `als`、`als_camera`、`als_extras` 并列。关卡、环境、UI 分别对应 `als_extras/levels`、`environment`、`ui`；移植验收与诊断归 `tools`。完整对应关系见 [源码目录说明](src/README.md)。

### 依赖指向（M0.2）

项目使用 Bevy 0.20.1、Avian 0.8 和 Enhanced Input 0.27。以下 fork 在根 `Cargo.toml` 中锁定到提交。

| crate（版本） | fork git 地址 |
|---|---|
| bevy_animation_graph 0.11.0 | `https://github.com/millionart/bevy_animation_graph`（codex/bevy-0.20） |
| bevy_gameplay_tag 0.3.0 | `https://github.com/millionart/bevy_gameplay_tag`（codex/bevy-0.20） |
| bevy-tnua 0.33.2 | `https://github.com/millionart/bevy-tnua`（codex/bevy-0.20） |
| bevy-tnua-avian3d 0.13.1 | `https://github.com/millionart/bevy-tnua`（codex/bevy-0.20） |

- fork 改动必须独立小步提交并可回馈 upstream（agents.md 规则 4）；任何时候禁止 vendor 拷贝插件源码。
- 本地源码调试时可用 `[patch.crates-io]`/path 覆盖，但路径只经 gitignored 本地配置或环境变量注入，禁止提交进仓库（agents.md 规则 7）。

## 状态

**M0–M11 与 BF102 已完成并归档**：详见 [归档清单索引](docs/archive/archivedchecklist.md)（逐项 Gate 证据）与 [docs/parity-report.md](docs/parity-report.md)（与 ALS-Refactored 差异对照）。

## 布娃娃物理边界

ALS-Refactored 使用 UE Chaos。本项目使用 Avian 的原生碰撞与接触求解，几何接触由 Parry 计算。布娃娃的接触点、法线、处理顺序、摩擦结果及碰撞后的姿态和轨迹可以与源工程不同。布娃娃地面球体扫掠采用 Avian 原生查询的命中距离。

布娃娃仍须正常倒下、与场景碰撞并起身。碰撞体、关节和必要的碰撞过滤继续提供这些行为。行走、上下台阶、攀爬、脚部 IK 及其他普通角色移动和动画功能继续按 ALS 的既定目标验证。这项豁免仅适用于布娃娃碰撞的引擎差异。

每个刚体的 CCD 开关映射到 Avian `SweptCcd` 的过滤配置。当前 `PA_Als` 的 21 个刚体使用空过滤，关闭扫掠 CCD。开启的刚体使用 Avian 的原生线性扫掠。

## 快速上手

Mac 与 Windows 共用 PowerShell 7、Rust 和 Python 3.10+。Mac 还需要 Xcode；UE 原生读取器使用对应平台的 UE 5.8.3 安装。PowerShell 安装见 [官方说明](https://learn.microsoft.com/powershell/scripting/install/installing-powershell)。

首次准备 Python 环境（Mac 将第一条的 `python` 换成 `python3`）：

```sh
python -m venv .venv
```

在 PowerShell 中安装工具依赖；脚本会优先使用当前工作区的 `.venv`：

```powershell
& (./tools/python.ps1) -m pip install -r tools/requirements.txt
```

将 `tools/env.example.json` 复制为 `tools/env.local.json`，填写本机 `ue_project` 和 `ue_cmd`。Mac 的命令位于 `Engine/Binaries/Mac/UnrealEditor-Cmd`，Windows 为 `Engine/Binaries/Win64/UnrealEditor-Cmd.exe`。`docs/env.md` 只记录环境，不参与工具配置；这两个本地文件均不提交，换机器时分别配置。`.venv` 与编译产物也不能跨系统复用。

源 ALS 工程也需要为当前平台构建 `ALSRefactoredEditor`；复制来的 `Binaries/Win64` 不能供 Mac 加载。本机保留的旧 `BlueprintNodeExporter` 只在 Windows 安装，其 `.uproject` 条目使用 `"PlatformAllowList": ["Win64"]`，工程 `TargetPlatforms` 同时列出 `Windows` 和 `Mac`。原生 reader 自己的插件由工具自动构建。

```powershell
pwsh -NoProfile -File ./run_manual.ps1
pwsh -NoProfile -File ./als_graph_editor.ps1
pwsh tools/run_animation_visual_validation.ps1 -Case startup -CaptureCount 0 -FunctionalOnly
pwsh tools/run_animation_visual_validation.ps1 -Case grounded -CaptureCount 0 -FunctionalOnly
```

- Windows 仍可双击原有 `.bat`；它们调用同名 `.ps1`，两边都使用工作区内的 `target_game` 与默认2个构建任务。
- 工具回归：`pwsh tools/run_ue_source_reader.ps1 -SelfTest`，以及 `& (./tools/python.ps1) -B tools/tests/test_platform_tools.py -v`。
- `manual_playground` 是交互运行与自动场景共用入口；公共 runner 的 `-Case` 选择相机、布娃娃、IK、运动等场景。`-FunctionalOnly` 验证功能，报告保留性能预算结果。
- 单元回归按改动选择 `cargo test --jobs 2 --lib <filter>`，例如 `als::motion::` 或 `extensions::root_graph_boundaries::tests`。全量回归使用 `cargo test --jobs 2`。独立的 Rig 算法测试可在插件仓库运行。 [Bevy Control Rig](https://github.com/millionart/bevy_control_rig)
- [examples 说明](tools/examples/README.md) 区分当前入口、静态合同和专项诊断；[tools 说明](tools/README.md) 列出保留的导入、转换与验收工具。
- `tools/tmp/saved/als_debug.ron` 保存手动运行的调试显示类别，不保存主显示开关；自动 Gate 不读写它。此目录不入 Git，删除该文件会重置类别选择，下次手动切换类别时重新生成。

### 自定义 Overlay

在 `assets/ALS/Data/als_gameplay_tags.json` 登记新标签（例如 `Als.OverlayMode.CustomRifle`），并在 `assets/ALS/Character/overlay_actor.runtime.json` 添加同名映射。可复制现有条目，再选择生成图、混合时间和物件配置；源导入器会补齐映射所需的 Tag 注册，并保留已有自定义标签。

重新启动 `run_manual.ps1` 后，Q 菜单会从 Tag 管理器枚举标签；无需新增 Rust 枚举变体。代码入口使用 `OverlayState::new` 和 `AlsStateRequest.overlay_state`，库由 `OverlayLibraryHandle` 选择。图和物件共同消费加载后的库，更新同一标签的配置也会刷新物件及图绑定。

自定义 LocomotionMode 使用 `LocomotionState::new("Als.LocomotionMode.Climbing")`，通过 `AlsStateRequest.locomotion_state` 传入；RON 数据使用 `Named("Als.LocomotionMode.Climbing")`。原有 `LocomotionState::Custom` 保留物理控制含义并映射为空 Tag。命名入口保留身份，不自动实现对应的物理动作或动画分支。

## 许可

ALS-Refactored 为商业资产，其内容不进入本仓库；本仓库自有代码 MIT OR Apache-2.0。
