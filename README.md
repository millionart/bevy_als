# bevy_als

用 Bevy ECS 复刻 [ALS-Refactored](https://github.com/Sixze/ALS-Refactored)（v4.18，UE 5.8）的高级运动系统。

- **不做代码级移植**：以 [bevy_animation_graph](https://github.com/mbrea-c/bevy_animation_graph)（替代 AnimBP）与 [bevy-tnua](https://github.com/idanarye/bevy-tnua)（替代 CharacterMovement）为基座，按需扩展。
- **不含网络同步**。
- 执行规则见 [agents.md](agents.md)，执行清单见 [CHECKLIST.md](CHECKLIST.md)（M0–M11 模块 + Gate），启动说明见 [goal.md](goal.md)。
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

**M0–M11 全部完成**（2026-08-24）：详见 [CHECKLIST.md](CHECKLIST.md)（逐项 Gate 证据）与 [docs/parity-report.md](docs/parity-report.md)（与 ALS-Refactored 差异对照）。

## 快速上手

```bash
cargo run --example locomotion_gate   # M5 全流程：起跑/急停/原地转身/蹲起（ALS 数值）
cargo run --example actions_gate      # M8 综合动作：冲刺前滚/助跑跳/停步距离匹配
cargo run --example camera_gate       # M10 相机：贴墙收缩/aim 肩位切换/pitch 联动
```

- gate 例程读取 `ALS_BOOT_AUTO_QUIT_SECS=<秒>` 做脚本化自动退出；不带则手动交互。
- 测试：`cargo test --workspace`。

## 许可

ALS-Refactored 为商业资产，其内容不进入本仓库；本仓库自有代码 MIT OR Apache-2.0。
