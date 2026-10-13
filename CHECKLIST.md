# bevy_als — 执行清单

> 本文件是唯一权威执行清单；执行纪律见 `agents.md`（仅规则），启动方式见 `goal.md`。
> 完成一项即勾选并一次提交：实现 → 自测 → 勾选 `[x]` → `git commit -m "<前缀><编号>: <条目摘要>"`（勾选与实现同一提交）。
> 基座模块提交前缀沿用 `M<编号>`；缺陷修复清单用 `BF<编号>`。
> 已归档清单：[archivedchecklist.md](docs/archive/archivedchecklist.md)

## 背景与基础事实（已核实，持续有效）

- 本机 bevy 源码：`0.19.1`（release-0.19.1）。`bevy_animation_graph` 0.11.0 与 `bevy-tnua` 均基于 Bevy 0.19，兼容。
- bevy_animation_graph 已有节点：ClipNode、ChainNode、BlendNode（含骨骼 mask/Difference/Additive）、BlendSpace1D/2D、布尔运算节点等；图资产 RON 序列化、支持编辑器热重载；Pose 以 `BoneId` 路径表达；RootMotionMode 提取已可用（M8.8）。曲线/ramp/距离匹配等由 als-ext 经公开扩展点补齐。
- bevy-tnua 已有 basis：Walk（coyote time）、Jump（高度/悬挂控制）、Crouch、Dash、Climb、Knockback、WallSlide；物理后端 avian3d 0.12.1 / rapier3d 0.17.0。
- ALS-Refactored 关键结构：4 模块（ALS / ALSCamera / ALSExtras / ALSEditor）；状态体系用 GameplayTags（非枚举）；自定义 AnimNode_CurvesBlend；ControlRig 输入结构 `AlsControlRigInput`；IK 相关骨骼 `foot_l/foot_r`、`ik_foot_l/ik_foot_r`、`VB foot_l/VB foot_r`；关键曲线名 `LayerHandLeft/Right`、`HandLeftIk/HandRightIk`、`FootLeftIk/FootLeftLock/FootRightIk/...`；动画资产分组：Grounded / Air / Actions / Transitions / TurnInPlace / RotateInPlace / Overlays / View / Base。
- 本地环境路径、引擎版本等隐私信息见 `docs/env.md`（已 gitignore）。

## 附录 A：与 ALS 的刻意差异（非缺陷）

| ALS | bevy_als | 理由 |
|---|---|---|
| AnimBP 状态机 | 源合同生成 RON/FSM，由公开节点与 ECS 输入接线执行 | 保留源状态、转换及求值生命周期 |
| GameplayTags | `bevy_gameplay_tag` 的真实层级 Tag | 保留 UE Tag identity 与 exact/hierarchical query 语义 |
| ControlRig | 离线编译源 RigVM 程序，在源 ControlRig 节点中逐实例执行 | 保留 Construction/Forward Solve、当前 Pose/Curve 及图内求值位置 |
| Root Motion | 图内 `RootMotionMode` 提取 + `RootMotionOutput` 驱动位移（M8.8；数据经 M8.6 回填） | fork 已支持提取，M0.5 导出缺根轨道由 M8.6 离线回填，不再用曲线等价物 |
| Curves（UBT 资产） | als-ext CurveAsset + 采样节点 | 见 M2 |
| w_als_hud Overlay 菜单世界锚定头顶骨骼 | 菜单停靠 HUD 面板左侧（BF6b 用户决策 2026-08-25） | 解除 UI 对骨骼/相机投影的耦合 |
| W_Als_Hud 使用 Roboto 字体 | Bevy 默认字体（BF6b 用户决策 2026-08-25） | 字体非功能项，不移植 |
| L_Als_Playground TextRender 使用 RobotoDistanceField 与描边材质 | Bevy 默认字体布局/atlas 生成的 `Mesh3d` 世界字形四边形（BF-Scene 用户决策 2026-08-26） | 不带入字体资产；保留文字内容、颜色、世界锚点与固定朝向，不做相机 billboard |
| S_Als_UI_Click 菜单点击音 | 不复刻（BF6b 用户决策 2026-08-25） | WAV 解码需 bevy `wav` feature（hound 依赖），用户否决新增依赖 |
| UE Shift+数字仍触发玩法Action | Shift+数字用于Debug时仅抑制对应数字binding的玩法动作 | BF9用户决策（2026-08-25；4018824），手柄和非数字binding保持源行为 |

## 附录 B：风险与后置项

- **资产格式决策（2026-08-21）**：Bevy 官方推荐 glTF；M0.5/M0.6 管线定为 UE → glTF（Interchange 导出）→ bevy_animation_graph 自带的 glTF 导入器（GraphClip/Skeleton）。**不引入 bevy_ufbx**：其 v0.19.0（bevy 0.19 兼容）虽支持蒙皮数据，但动画曲线未接入 Bevy 动画系统、无社区验证，且 FBX 中转对 ALS 曲线/虚骨/单位换算风险更大；仅当未来需要直接加载 FBX 时再评估。

- bevy_animation_graph additive/mask 语义若与 ALS layering 不完全一致，在 M6 前验证并决定扩展方式（优先 als-ext 扩展点）。（已于 M6.1 验证通过，记录备查）
- Distance matching / motion matching 类高级特性仅实现 Stop 所需子集（M8.3）。
- ALSExtras（AI 控制器示例）不在范围内。
- 音频（Metasound）、Niagara 特效仅保留挂钩点。ALSCamera 完整移植已纳入 BF116.01。

## BF142 天空接入后的场景亮度与曝光

用户于2026-10-13明确要求处理天空接入后的阴沉观感。范围为原Playground天空材质输出、SkyLight环境补光与曝光显示链；截图0、性能验收暂停。先按当次本地源事实定位差异，再使用现有引擎功能，不凭观感提高太阳或添加补偿。

- [x] BF142.01 源照明/曝光与实际渲染入口静态闭环。开始：2026-10-13 08:49 +08:00；预估结束：2026-10-13 09:14 +08:00；实际结束：2026-10-13 09:13:06 +08:00；偏差−54秒，source/static Gate PASS，无需重估。依据本轮原生World读取80.603秒、原天空材质约75节点已有审计器、当前Bevy原生AutoExposure/EnvironmentMap与BF139-P2-18同类数据闭包记录，先以25分钟完成源参数/时钟/单位、HDR/材质/相机消费者、最小失败及后续可独立验收拆分。Gate：fresh World/Camera与UE实际默认、locked Bevy执行语义、必要的读工具补齐、自测及明确剩余边界；本叶不将静态通过称视觉修复。
- [ ] BF142.02 曝光/显示与环境补光修复。依赖.01 Gate，启动时按证实闭包拆为各自不超过60分钟的独立子项并登记时点。Gate：源驱动独立target数据、最小失败→修复、消融/字节恢复、同target_game正式构建、原Playground数值渲染与公共功能Gate；旧固定显示近似仅在源/症状/消费者共同证明的边界有限复开。

活动摘要（08:49）：工作区基线`93ac4a47`且干净。刚读取的World与原插件/安装副本字节一致：太阳3lux、SkyLight强度2/Stationary/CapturedScene/非实时/下半球黑，PostProcessVolume只覆盖MotionBlur；当前固定EV1002/contrast1.05来自旧显示近似，天光用常量Ambient2。下一步核对源相机后处理覆盖、曝光算法和天空进入HDR的真实路径；原始导出保持不可变。

source/static摘要（09:13）：fresh Camera Blueprint读取34.221秒证实PostProcessWeight=0，Sky Material读取69.379秒证实Surface/Opaque/Unlit；UE默认Histogram、10%–90%、速度3/1、range −10..20、18%中灰与bias1已闭合。四档均匀亮度反例证明固定EV2不能维持源中灰0.36；lens/native基准换算单元复算通过，原始记录/源码快照及哈希在`target/bf142/source_contract.json`。实际入口没有Hdr/AutoExposure，天空绕过普通材质曝光/非HDR tone mapping，SkyLight未接入。后续分别闭合曝光/HDR显示与一次性128天空捕获→原生环境照明；沿用源材质/网格算法，UE Lumen和两引擎tonemapper/filter实现差异保留能力边界，不把任意亮度补偿当源算法。当前仅source/static完成，尚未宣称观感修复。

