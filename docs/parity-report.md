# ALS-Refactored 移植差异

对标基线为本地 ALS-Refactored v4.18 和 UE 5.8。目标运行在 Bevy 0.20.1、Avian 0.8 和 Tnua 0.33.2。验收以 Playground 的实际角色、场景和动作路径以及源合同测试为准。

| 范围 | Bevy 实现与验收依据 |
| --- | --- |
| ALS 角色与运动 | ECS 角色组件保留输入、姿态、步态、旋转、移动基座和地空切换的语义。Tnua 和 Avian 提供控制器与物理基础，ALS 保留运动策略、根运动以及翻滚、攀爬、布娃娃和起身的调度。参见 BF144、BF146 及 [布娃娃交接](audit/bf150/ragdoll-source/README.md)。 |
| 动画图与 Control Rig | 原生 AnimBP 的状态机、分层、同步、曲线、事件和 Slot 合同生成 RON/FSM。ECS 提供角色输入，图内 Rig 节点在原阶段执行离线编译的 RigVM 程序。GameplayTag 保留层级和匹配语义。参见 [BF115](archive/2026-09-12/BF115-2026-09-12.md)、[Rig 插件验收](audit/bf149/02/README.md)和[分层复验](audit/bf150/overlay-test-recheck/README.md)。 |
| ALSCamera | 相机组件使用生成的图与曲线、基座继承、场景探测和震动配置。参见[首次求值验收](audit/bf116/01c2a-camera-first-evaluation/README.md)和[相机查询回归](audit/bf150/camera/runner.json)。 |
| ALSExtras | 示例保留 Playground、PlayerStart、环境、HUD、Overlay 模式菜单和本地操控。接收器材质执行脚印贴花，源通道与运行采样保留专项验证。参见[资产布局](asset-layout.md)和 [Footprint 验收](audit/bf150/scene-regressions/runtime.json)。 |

## 保留的差异

- 默认重力采用 Bevy/Avian 的 9.81 m/s²，源默认值为 9.80 m/s²。比较运动算法时两侧使用相同输入。
- 布娃娃用 Avian 与 Parry 的原生接触求解。允许接触点、法线、顺序、摩擦以及由此产生的物理轨迹不同。倒下、碰撞、起身与输入交接仍验收，普通运动与动画不适用该豁免。详见[布娃娃边界](../README.md#布娃娃物理边界)。
- Overlay 菜单停靠 HUD 左侧。HUD 和 Playground 文字使用 Bevy 字体，场景文字保持原世界位置和固定朝向。菜单点击音未移植。Shift＋数字用于调试时抑制对应数字键的玩法绑定，手柄及其他绑定仍有效。
- 距离匹配只实现 ALS 停步使用的部分。网络同步、ALSExtras AI 和 UE 专属 ALSEditor 工作流在原范围之外。MetaSound 和 Niagara 仅留事件接线点。

## 验收口径

历史源审计以具体资产、输入和消费阶段为单位。通过的源合同与场景覆盖不保证任意输入逐位相同，也不替代尚未覆盖的场景。性能数据对应报告中的二进制、平台、时钟和场景。历史 Mac 测试记录既包括通过的完整动作性能回归，也包括 Metal 冷管线编译的启动长帧。Windows 启动脚本与平台夹具已验，现轮没有 Windows 真机结果。

BF154.01 通过串行全量测试及真实 Playground 启动和 Default Overlay 验收。两场完整性能通过的结果对应缓存就绪后的同一二进制，首次进程的长帧仍保留。见[验收基线与复跑命令](audit/bf154/baseline/README.md)。

插件整理后的复验覆盖默认依赖的独立 Bevy 项目、工作区 2,128 项单元测试与 37 项集成测试，以及原 Playground 的启动、Default Overlay、Footprint、布娃娃与起身。Footprint 首次运行通过功能与 GPU 采样，暖态复跑通过完整预算。见[插件交付回归](audit/bf154/plugin/README.md)。
