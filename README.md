# Advanced Locomotion System for Bevy

`bevy_als` 是面向 Bevy 0.20.1 的第三人称角色运动插件，移植自 [ALS-Refactored](https://github.com/Sixze/ALS-Refactored)。`AlsPlugin` 提供地面与空中运动、蹲起、翻滚、攀爬、分层动画、脚部 IK、布娃娃、起身和第三人称相机。

## 刻意未移植的功能

| 原项目功能 | 本项目范围 |
| --- | --- |
| 网络复制、Iris 与多人运动预测 | 单机角色系统 |
| ALSExtras 的 AI 示例 | 本地玩家角色与场景示例 |
| Unreal 编辑器专用资产操作、骨架设置与动画修改器 | Bevy 运行时加载已转换资产 |
| MetaSounds、Niagara 与菜单点击音 | 保留脚步等事件接线 |
| 通用距离匹配与运动匹配 | 实现 ALS 停步所需的距离匹配 |

## Unreal 依赖与 Bevy 对照

| Unreal | Bevy |
| --- | --- |
| Animation Blueprint、状态机与动画层 | [bevy_animation_graph](https://github.com/millionart/bevy_animation_graph) |
| Control Rig 与 RigVM | [bevy_control_rig](https://github.com/millionart/bevy_control_rig) |
| CharacterMovementComponent | [bevy-tnua](https://github.com/idanarye/bevy-tnua) 与 Avian 适配 |
| Chaos、碰撞查询与布娃娃求解 | [Avian](https://github.com/avianphysics/avian) 与 Parry |
| Gameplay Tags | [bevy_gameplay_tag](https://github.com/millionart/bevy_gameplay_tag) |
| Enhanced Input | [bevy_enhanced_input](https://github.com/simgine/bevy_enhanced_input) |
| 角色、相机、渲染与 UI 组件 | Bevy ECS、AssetServer、PBR 与 UI |

## 对依赖的补充

| 扩展 | 补充内容 |
| --- | --- |
| `animation_graph_ext` | 序列时钟、同步组、可中断转场、Slot、曲线、分层节点和图内 Rig 执行。 |
| `control_rig_ext` | ALS 专用 Rig 单位，包括脚部探测、足部偏移和旋转分配。 |
| ALS 角色运动与 `tnua_ext` | ALS 的步态、旋转、台阶、移动基座、动作根运动和物理时序。 |
| `avian_ext` | 场景查询、碰撞过滤与骨骼刚体交接。布娃娃接触继续使用 Avian 原生求解。 |
| `bevy_ext` | 脚印接收材质、贴花属性混合及场景渲染辅助。 |
