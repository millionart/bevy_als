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

配套模块：[millionart/bevy_controlrig](https://github.com/millionart/bevy_controlrig)（私有）——ControlRig 等价的 IK 子系统（two-bone/foot/pelvis/hand IK），独立仓库、经 crate API 依赖。

## 状态

🚧 规划阶段：模块清单已定（M0–M11），尚未开始实现。

## 许可

ALS-Refactored 为商业资产，其内容不进入本仓库；本仓库自有代码 MIT OR Apache-2.0。
