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

## 附录 B：风险与后置项

- **资产格式决策（2026-08-21）**：Bevy 官方推荐 glTF；M0.5/M0.6 管线定为 UE → glTF（Interchange 导出）→ bevy_animation_graph 自带的 glTF 导入器（GraphClip/Skeleton）。**不引入 bevy_ufbx**：其 v0.19.0（bevy 0.19 兼容）虽支持蒙皮数据，但动画曲线未接入 Bevy 动画系统、无社区验证，且 FBX 中转对 ALS 曲线/虚骨/单位换算风险更大；仅当未来需要直接加载 FBX 时再评估。

- bevy_animation_graph additive/mask 语义若与 ALS layering 不完全一致，在 M6 前验证并决定扩展方式（优先 als-ext 扩展点）。（已于 M6.1 验证通过，记录备查）
- Distance matching / motion matching 类高级特性仅实现 Stop 所需子集（M8.3）。
- ALSExtras（AI 控制器示例）不在范围内。
- 音频（Metasound）、Niagara 特效仅保留挂钩点。ALSCamera 完整移植已纳入 BF116.01。

## BF132 — 浏览器第七轮审阅的全部 49 项反馈

来源：内置浏览器《Review P2移植差异》的第七轮 `review.current.md` 与 `findings.current.json`，2026-10-02 读取。审阅基线 `5e32010486ade738b6ed2c75f6b5387a53354296`；本批起始 HEAD `9942ca3`。对标 ALS `b754d6f0f2bb03741d301f8fb88077ebfe561e17`、UE `396c9f059903aed5fec78ecd3d437a40c6415368`。保留全部原编号：R71 的 18 项与 R5E 的 31 项，共 49 项 P2；撤回的 R71-06 不计入。用户已将全部 49 项选为当前修复任务。

执行：按以下模块与依赖顺序逐项重新提取本地 source、核对当前 target 与实际消费者。已有修复只在相同触发条件的当前 Gate 通过后核销；确认残留时先留下最小失败反例，再修复根因、消融、回归与真实入口验收。每项通过后单独勾选和提交。网页隔离模型不代替当前 Rust/Playground Gate；不得凭标题复开旧条目。

估时：参考 BF131 登记及同类 S1/S2/S3 实际记录，逐项拆为源合同与失败反例、最小修复与消融、真实入口验收；每个窗口不超过 60 分钟。排队项不冒记开始时间，开始时按 fresh source 闭包填写开始/预估结束；超过窗口与误差按 agents.md 复盘、拆分后继续。修改转换规则须重转全部安装 AnimBP 及递归依赖并审阅完整新旧 diff。动画行为采用 animation-graph-equivalence 公共 Gate。

### 当前 Avatar、播放器、相机与材质生命周期

- [ ] **BF132.R71-03 — 保留 avatar 根实体重建场景时，新播放器漏掉 PostPhysicsAnimation 标记**。Gate：同根重建的新播放器在正确阶段求值，旧播放器停用与首次加载控制通过。
- [ ] **BF132.R71-04 — 同角色 source camera 共写 frame/curves，旧停用组件可使当前相机退回 fallback**。Gate：当前相机独占其输入与输出，保留旧组件、切换/停用与重建通过。
- [ ] **BF132.R71-07 — 脚步通知丢失发出事件的 mesh/player 身份，旧图事件落到新 avatar 脚骨**。Gate：旧实例事件不作用新 Avatar，当前实例正常派发，两个拆装顺序通过。
- [ ] **BF132.R71-12 — ragdoll 脱离播放器缺少随角色正常销毁的清理路径**。Gate：角色销毁及两种父子拆除顺序清理所属播放器、物理体与输入；其他角色保持正常。
- [ ] **BF132.R71-14 — AvatarMaterialApplied 阻止材质改写和同根重建后的重新绑定**。Gate：材质修改、同根重建与当前 Avatar 替换正确绑定，稳定帧不重复重建。
- [ ] **BF132.R71-09 — 不接收贴花的角色策略错误依赖可选的材质覆盖组件**。Gate：没有材质覆盖时角色仍不接收贴花，环境接收器与可选覆盖控制通过。
- [ ] **BF132.R5E-05 — 翻越姿态发布选择第一个同角色 avatar，而不是当前 assembly 绑定的播放器**。Gate：保留旧 Avatar 时只向当前 Assembly/player 发布，替换与恢复通过。
- [ ] **BF132.R71-19 — 零关节布娃娃不发送 RagdollSpawned，导致自身物理体漏出相机排除集合**。Gate：零/非零关节均发布一次实际生成体事件，相机排除及替换清理通过。

### 角色运动、碰撞、出生与默认输入

- [ ] **BF132.R5E-01 — 地面超速分支提前返回，刹车后未继续施加转向输入加速度**。Gate：超速/同向/转向/无输入按源 CalcVelocity 顺序消费刹车与加速度。
- [ ] **BF132.R5E-02 — 移动平台速度混入期望速度朝向，VelocityDirection 朝平台合成速度转身**。Gate：平台携带与角色相对运动分别消费，碰撞前 yaw 与低速回退符合源合同。
- [ ] **BF132.R5E-04 — 起身空间检查把普通 Sensor 的非阻挡重叠当成头顶障碍**。Gate：Sensor、碰撞层不相交与实际阻挡低顶分别得到源规定结果。
- [ ] **BF132.R5E-06 — 起身缺少利用地面间隙下移后重试的源分支**。Gate：Grounded 有可用间隙时下移重试成功，间隙不足与真实阻挡保持蹲姿。
- [ ] **BF132.R5E-07 — 物理后地面高度修正未做胶囊 sweep，可把已站立角色推入低顶**。Gate：补偿位移按源 sweep 截断，低顶不穿透，无遮挡地面高度控制通过。
- [ ] **BF132.R5E-08 — 防穿地恢复使用未缩放半高，误把未接地的小角色向上抬起**。Gate：不同有效胶囊 scale 的离地/接地/穿地恢复保持源世界空间语义。
- [ ] **BF132.R5E-09 — 空中有输入时仍按 Tnua 目标速度纠偏，额外刹掉原方向动量**。Gate：空中转向输入只施加源允许加速度，原方向动量、无输入与超速控制通过。
- [ ] **BF132.R5E-10 — 普通自由下落链缺少源终端速度限制，预测用 -40 不能代替实际速度限制**。Gate：实际自由下落速度使用当前 PhysicsVolume 有效终端速度，普通/覆盖与预测独立。
- [ ] **BF132.R5E-11 — 出生解重叠只选单个最短轴，丢掉多障碍所需的组合 MTD**。Gate：单/多障碍组合调整与源重试顺序一致，成功与不可恢复出生点区分。
- [ ] **BF132.R5E-12 — 默认高位向下探地把合法室内出生点搬到屋顶**。Gate：合法室内出生位置保留，确有重叠/落地需要时按源查找，不以屋顶替换。
- [ ] **BF132.R5E-13 — 补充完整半径地面检测绕过源缩小半径停靠复检**。Gate：边缘/侧墙接触不能绕过缩半径复检，可靠地面与已接地控制通过。
- [ ] **BF132.R5E-14 — 手柄 Select/Start 的默认 ToggleUI 与 Slomo 绑定相反**。Gate：fresh 源输入资产与运行绑定逐键一致，两个动作各自触发。
- [ ] **BF132.R5E-15 — 平台变换跟随的 sweep 把普通 Sensor/不相交碰撞层当作阻挡**。Gate：同一路径区分非阻挡重叠与有效 Block，平台移动/旋转及真实障碍通过。
- [ ] **BF132.R5E-16 — 移动基座起跳首个长帧漏掉整帧重力速度补偿**。Gate：首帧长 Delta、普通帧与脱离平台使用源整帧重力与相对速度合同。
- [ ] **BF132.R5E-17 — 有效出生点尚在稳定采样时被 GameMode 误判为终止入场失败**。Gate：进展、稳定等待与终止失败分别处理，正常出生不提前报失败且有 watchdog。
- [ ] **BF132.R5E-18 — 跨步抬升后遇到后续前向阻挡即放弃，丢弃源允许的部分前移**。Gate：抬升/部分前移/下落按源顺序落地，完全阻挡与不可跨步控制通过。

### 动画通知、动作、图更新与同步

- [ ] **BF132.R71-08 — Control Rig 关闭输入曲线传递时连带丢弃非曲线上下文**。Gate：关闭曲线仅影响曲线，事件、根运动、资产采样与其他上下文保留。
- [ ] **BF132.R71-15 — Rig 接受非 Visibility 的源通道，却在实际求值时拒绝该通道**。Gate：转换支持资格与实际命名查询层一致，Visibility/Camera 同几何不同响应通过。
- [ ] **BF132.R71-13 — 翻滚根运动转换遗漏当前网格尺度并依赖固定朝向**。Gate：当前 Mesh transform 的平移/旋转/scale 正确作用根运动，同根替换与直线控制通过。
- [ ] **BF132.R71-18 — 翻滚把已停止 Montage 的淡出尾段继续视为不可重入，丢弃源端允许的新翻滚请求**。Gate：停止/播放/淡出区分，尾段重入被接受且旧实例完成清理。
- [ ] **BF132.R5E-19 — 根运动缩放通知在 Active 阶段反复夺回数值，偏离源 Begin-only 生命周期**。Gate：重叠通知、外部写入与 End 条件恢复，Active 不夺回缩放。
- [ ] **BF132.R5E-20 — Pivot 请求只携带角色，下一帧用新速度重新判断激活门槛**。Gate：保存通知当次输入的严格门槛结果，跨阈值、Initialize/Reset 覆盖通过。
- [ ] **BF132.R5E-21 — 过渡 montage 把负 PlayRate 钳成正 EPSILON，倒播近乎变为冻结**。Gate：有符号采样、方向相关终止/寿命、零速率、反向通知与曲线时钟通过。
- [ ] **BF132.R5E-22 — 设置落地入口通知忽略空标签，不能清除既有 FromRoll**。Gate：空/自定义/FromRoll 标签完整赋值，独立 Reset 与真实入口分支通过。
- [ ] **BF132.R5E-23 — 翻滚首次通知采样丢弃已推进的首个时间区间**。Gate：首帧长 Delta、不为零的起播、seek、资产迟到与后续无重复通知通过。
- [ ] **BF132.R5E-24 — 移动下蹲交叉混合期间，仍相关的 Standing 分支使用陈旧输入**。Gate：T19 淡出期 Standing 按实际相关性刷新 Gait/stride/rate，结束后生命周期与脚锁归属正确。
- [ ] **BF132.R5E-25 — 缺少源默认可见性驱动的动画停更与恢复生命周期**。Gate：最近渲染/多视图、隐藏后的图停更、Montage 继续与 PendingUpdate 恢复分别符合源策略。
- [ ] **BF132.R5E-26 — 自动过渡的循环判定使用无符号帧时长，倒播误触发或漏触发跳转**。Gate：有符号实际播放增量判定正反向跨循环，保留源剩余时间与淡出公式。
- [ ] **BF132.R5E-27 — 显式暂停播放器后，生成状态机仍被全局动画 Delta 推进**。Gate：pause、PlayOneFrame、seek、custom dilation、世界暂停与同步时钟各自正确传递。
- [ ] **BF132.R5E-28 — 通知 LOD 过滤设置被导入并保留，但运行时未执行**。Gate：Queued/BranchingPoint 区分，合法阈值 0/1、实际 LOD 与状态 End 清理通过。
- [ ] **BF132.R5E-29 — 混合分支更新权重和缓存仲裁未对标，真实起跑状态选择错误**。Gate：Land Movement 内部 0.25 混合以最大请求 0.75 更新 Grounded 缓存，真实 T8/T9、遍历顺序/竞争请求通过。
- [ ] **BF132.R5E-03 — 无共同标记的新 leader 被强制接到旧组相位，遗漏源重同步资格**。Gate：按源玩家类型/入组资格重同步，无共同 marker、有 marker 与新 leader 对照通过。
- [ ] **BF132.R5E-30 — 无标记 BlendSpace 回退仍使用最高权重样本时钟，丢失加权归一化速度**。Gate：以带符号加权归一化速度推进，无 marker 多样本、倍率/时长/循环与有效 marker 对照通过。

### PhysicsAsset、关节求解与布娃娃回读

- [ ] **BF132.R71-01 — 全锁线性关节的速度约束门限与源 cached SIMD 三轴块语义不一致**。Gate：确认实际源求解器分支；全零/单轴/多轴位置 lambda、速度变化与非零锚臂保留三轴合同。
- [ ] **BF132.R71-02 — 多权重 ragdoll 回读直接相加四元数，符号相消后产生无效旋转**。Gate：q/-q 等价旋转混合保持单位/有限，部分回读及零权重控制通过。
- [ ] **BF132.R71-05 — 源要求随缩放更新质量，目标却固定生成的 Override 质量**。Gate：effective Body scale、密度/质量幂与 Override 开关分别按源生成及重算。
- [ ] **BF132.R71-10 — 脱离后的 ragdoll 物理配置仍读取原 avatar 宿主缩放**。Gate：脱离前后实际 root/world scale 正确重配，宿主变化不污染脱离体。
- [ ] **BF132.R71-11 — 默认 PA_Als 的 cached 双轴摆动限制被替换成更窄的椭圆边界**。Gate：fresh 实际 cached/ordinary 分支与默认 PA 配置共同证明正确边界，斜向/轴向与内外控制通过。
- [ ] **BF132.R71-16 — PhysicsAsset 的 bUseCCD 被接受但未传入 ragdoll 的实际 CCD 配置**。Gate：源开关贯穿生成/安装/重建与实际高速碰撞，不以字段存在宣称实现。
- [ ] **BF132.R71-17 — PhysicsAsset 的 SixDOF 逐轴锁定未生成或安装有效约束**。Gate：SixDOF 项目/实例有效模式及逐轴锁实际限制运动，自由轴与重建通过。
- [ ] **BF132.R5E-31 — Body 惯量条件化被丢弃；现有 Joint 质量条件化不是等价替代**。Gate：独立 solver 惯量消费 Body 标志、几何/约束臂及失效重算，物理质量保持源值；缩放、拓扑/动态切换和 Joint 条件化共存通过。

### 活动记录

- BF132 登记：开始 2026-10-02 02:37 +08:00；预估结束 2026-10-02 02:57 +08:00；实际结束 2026-10-02 02:50 +08:00；偏差 -7 分钟；结果：49/49 唯一原编号与第七轮全量索引一致，R71-06 排除，`git diff --check` 通过。依据：BF131 同类 37 项登记实际 12 分钟与当前完整 49 行索引；已完整读取第七轮报告与累计编号，JSON 下载未返回文件，后续通过预览核对具体历史触发条件。当前工作树初始干净；尚未把任何继承反馈或归档 Gate 记为本批通过。拆分：依上述模块顺序逐项 source/当前反例、最小修复/消融、真实入口 Gate；下一项 R71-03。
