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

本仓库以 **path 依赖**指向本机 fork（路径配置在根 `Cargo.toml` 的 `[workspace.dependencies]`）：

| crate（版本） | 本机 fork path |
|---|---|
| bevy_animation_graph 0.11.0 | `<fork-bevy_animation_graph>/crates/bevy_animation_graph` |
| bevy-tnua 0.32.0 | `<fork-bevy-tnua>` |
| bevy-tnua-avian3d 0.12.1 | `<fork-bevy-tnua>/avian3d` |

- fork 改动必须独立小步提交并可回馈 upstream（agents.md 规则 4）；任何时候禁止 vendor 拷贝插件源码。
- 其他机器克隆本仓库时，需按相同结构放置 fork（或将 `[patch]`/path 换成 git 依赖）。

## 状态

🚧 执行中：按 [CHECKLIST.md](CHECKLIST.md) 自 M0 起推进（当前进度见清单勾选）。

## 许可

ALS-Refactored 为商业资产，其内容不进入本仓库；本仓库自有代码 MIT OR Apache-2.0。
