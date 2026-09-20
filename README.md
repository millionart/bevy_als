# bevy_als

用 Bevy ECS 复刻 [ALS-Refactored](https://github.com/Sixze/ALS-Refactored)（v4.18，UE 5.8）的高级运动系统。

- **不做代码级移植**：以 [bevy_animation_graph](https://github.com/mbrea-c/bevy_animation_graph)（替代 AnimBP）与 [bevy-tnua](https://github.com/idanarye/bevy-tnua)（替代 CharacterMovement）为基座，按需扩展。
- **不含网络同步**。
- 执行规则见 [agents.md](agents.md)，当前执行清单见 [CHECKLIST.md](CHECKLIST.md)，已完成批次见 [归档索引](docs/archive/archivedchecklist.md)，启动说明见 [goal.md](goal.md)。
- 本地环境与 Unreal MCP 配置见 `docs/env.md`（隐私，不入库）。

## 基座插件 fork

| upstream | fork | 用途 |
|---|---|---|
| mbrea-c/bevy_animation_graph | [millionart/bevy_animation_graph](https://github.com/millionart/bevy_animation_graph) | 动画图（AnimBP 替代） |
| idanarye/bevy-tnua | [millionart/bevy-tnua](https://github.com/millionart/bevy-tnua) | 浮动角色控制器（CharacterMovement 替代） |

ControlRig 等价的 IK 子系统（two-bone/foot/pelvis/hand IK）位于本仓库 `crates/als-ext`，不单独建仓。

### 依赖指向（M0.2）

本仓库以 **git 依赖**指向公开 fork（配置在根 `Cargo.toml` 的 `[workspace.dependencies]`）：

| crate（版本） | fork git 地址 |
|---|---|
| bevy_animation_graph 0.11.0 | `https://github.com/millionart/bevy_animation_graph`（master） |
| bevy-tnua 0.32.0 | `https://github.com/millionart/bevy-tnua`（main） |
| bevy-tnua-avian3d 0.12.1 | `https://github.com/millionart/bevy-tnua`（main） |

- fork 改动必须独立小步提交并可回馈 upstream（agents.md 规则 4）；任何时候禁止 vendor 拷贝插件源码。
- 本地源码调试时可用 `[patch.crates-io]`/path 覆盖，但路径只经 gitignored 本地配置或环境变量注入，禁止提交进仓库（agents.md 规则 7）。

## 状态

**M0–M11 与 BF102 已完成并归档**：详见 [归档清单索引](docs/archive/archivedchecklist.md)（逐项 Gate 证据）与 [docs/parity-report.md](docs/parity-report.md)（与 ALS-Refactored 差异对照）。

## 快速上手

```bash
cargo run --example locomotion_gate   # M5 全流程：起跑/急停/原地转身/蹲起（ALS 数值）
cargo run --example actions_gate      # M8 综合动作：冲刺前滚/助跑跳/停步距离匹配
pwsh tools/run_animation_visual_validation.ps1 -Case camera -CaptureCount 0 -FunctionalOnly
```

- gate 例程读取 `ALS_BOOT_AUTO_QUIT_SECS=<秒>` 做脚本化自动退出；不带则手动交互。
- 测试：`cargo test --workspace`。

### 自定义 Overlay

在 `assets/als_gameplay_tags.json` 登记新标签（例如 `Als.OverlayMode.CustomRifle`），并在 `assets/character/overlay_actor.runtime.json` 添加同名映射。可复制现有条目，再选择生成图、混合时间和物件配置；源导入器会补齐映射所需的 Tag 注册，并保留已有自定义标签。

重新启动 `run_manual.bat` 后，Q 菜单会从 Tag 管理器枚举标签；无需新增 Rust 枚举变体。代码入口使用 `OverlayState::new` 和 `AlsStateRequest.overlay_state`，库由 `OverlayLibraryHandle` 选择。图和物件共同消费加载后的库，更新同一标签的配置也会刷新物件及图绑定。

## 许可

ALS-Refactored 为商业资产，其内容不进入本仓库；本仓库自有代码 MIT OR Apache-2.0。
