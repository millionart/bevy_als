# docs/parity-report.md — bevy_als vs ALS-Refactored (UE 5.8, v4.18) 差异对照

> 生成于 2026-08-24，对应 CHECKLIST M0–M11 全部完成状态。逐功能核对基于 ALS-Refactored 源码
> （AlsGameplayTags.h / AlsCharacter.cpp / AlsMovementSettings.h / AlsAnimationInstance 等）与
> 本仓库各模块 Gate 证据；刻意差异（非缺陷）与待优化项分开列出。

## 1. 架构级刻意差异（设计决定，非缺陷）

| ALS-Refactored | bevy_als | 理由 | 记录位置 |
|---|---|---|---|
| UE GameplayTags 状态体系 | 普通 Rust 枚举组件（LocomotionState/Stance/Gait/RotationMode/OverlayState），合法性经 Verdict(Accept/Reject/Queue) 系统约束 | 编译期检查、ECS 数据流 | M1.1–M1.2 |
| AnimBP 状态机图 | 显式 system 路由 + animation graph RON（blend space/FSM 由 fork 提供） | 无隐藏图执行逻辑 | 附录 A |
| ControlRig 运行时 rig | als-ext 分析式两骨 IK（cosine solver，骨骼实体 Transform 直写） | 无 rig 编译开销 | M7.2 |
| Root Motion（骨骼位移提取） | 曲线驱动位移：Roll 用解析钟形速度曲线（∫v dt≡3m 解析精确）；Mantle 用参数化三段曲线 | bevy_animation_graph 无 root motion 提取，避免侵入 fork | M8.2/M8.4 |
| AnimSequence 内嵌浮点曲线（UBT 资产内） | 自研 uasset→glTF 扩展 `ALS_float_curves` 重导出 + `bevy_gltf::GltfExtensionHandler` 运行时导入（als-ext::als_curves） | 引擎管线两端原生不透传浮点轨道 | M0.5b/M2.4/M7.8 |
| CurveFloat UBT 资产 | als-ext CurveAsset（.curve.ron，Linear/Step/Smooth 三采样） | 同上替代 | M2.1 |
| ALSCamera 独立模块 | 独立 `crates/als-camera` crate（spring arm 射线收缩/肩偏移/三模式/RON 参数） | 对齐模块边界 | M10.1–10.2 |

## 2. 行为级差异（功能等价或低配实现）

| 功能 | ALS 原版行为 | 本仓库实现 | 差距/偏离说明 |
|---|---|---|---|
| 急停（Stop） | 输入释放即播 QuickStop，按剩余转角选 L/R 支、play_rate 随转角缩放 | StartStopState Stopping 触发 + DistanceMatchNode 单边时间 warp（remaining 由实测位移闭环消耗，速度归零定格足触点） | 无双向 pose warp；预测分母为恒减速解析值，tnua 实际制动非恒减速（滑移 0.205m vs 预测 0.469m），对齐性不受影响 | 
| TurnInPlace | FSM 切换 Turn_90/180 动画播放 | 事件检测器（阈值/延迟/play_rate 对齐 AlsTurnInPlaceSettings）+ 脚部钉地 hold | 动画本体未接入图链（事件与钉地已生效），接入属增强项 |
| AimOffset | yaw 轴 Look_L/R 偏移动画 | yaw 由身体朝向承担（RotationMode/Aiming + RotateInPlace）；pitch 走 BlendSpace1D(Look_Down/Forward/Up) | 库内本就无 yaw 类偏移资产；语义由身体旋转等价 |
| Overlay 叠加 | LayerHand*/LayerArm* 曲线驱动动态加法 | 同名实测曲线经 ALS_float_curves 导入，分组动态 mask（SpineAdditive/ArmRight/HeadAdditive）逐帧生成 BoneMask | 手部 IK 目标（HandLeftIk/HandRightIk）已导入但 Rifle overlay 未实现（无持枪需求） |
| Foot IK | ControlRig 双骨 IK + pelvis 位移 + FootLock 曲线锁相 | 垂直 offset 补偿 + pin 锁存触地帧收敛目标；水平位移不锁定 | 横向滑动在急停/转身时依赖动画本身；完整水平锁相留待后续 |
| Roll 方向选择 | 输入相对视图八向分类 | 复用 classify 八向映射世界 yaw（±π/4 步进） | 等价 |
| Mantle 触发 | 专用 Mantle 输入 + Ledge 检测 | 低配自动触发（前向输入遇可翻台面），免按键；High/LowL/LowR 三类齐备 | 记录在案的输入偏离 |
| Ragdoll blend-out | GetUp_Front/Back 动画混合起身 | 退出沿直接对齐 pelvis body 位姿切回动画（无 GetUp 混合） | GetUp 资产已在库，混合起身属增强项 |
| 相机 FirstPerson | 完整第一人称 | 占位（eye 高度+臂长 0.05，无网格隐藏/手部姿态） | 明确占位 |
| 状态网络同步 | （ALS-Refactored 含 CMC 预测） | 不在网络范围（goal 边界） | goal 排除项 |
| AI（ALSExtras） | 示例 AI 控制器 | 不在范围 | goal 排除项 |

## 3. 性能对照

| 指标 | 目标 | 实测 | 结论 |
|---|---|---|---|
| 单角色动画图求值 | <0.5 ms（debug×3 放宽=1.5 ms） | release avg=1.286 ms / p95=1.521 / max=1.950（perf_gate 600 样本，locomotion_overlay 全图） | 达放宽线；未达原生 0.5 ms —— 优化项：节点合并、Context 分发降耗、mask 常量化 |
| debug 构建（参考） | — | avg=6.9 ms（仅记录，不判 Gate） | debug 无优化虚分发成本高 |

## 4. 数值保真核对（抽样）

| 参数 | ALS 值 | 本仓库 | 出处 |
|---|---|---|---|
| walk/run/sprint 速度 | 175/375/650 cm/s | 1.75/3.75/6.50 m/s | AlsMovementSettings.h |
| 加速/减速/摩擦 | 2000/1500/? cm/s² | 20.0/15.0/4.0 | AMC.cpp:139-141 |
| 胶囊站半高/半径 | 90/30 cm | 0.90/0.30 m | AlsCharacter.cpp:31 |
| 蹲速 | 150 cm/s | 1.50 m/s | AlsCrouchingSettings.h:13 |
| 跳跃 Z 速度 | 420 cm/s | 4.20 m/s（重力 1.0） | 工程默认 |
| 旋转速率 VD/Aiming | 800/1000 deg/s | 同 | AlsCharacter.cpp≈1574-1884 |
| TurnInPlace 阈值/180 阈值/play_rate | 45°/130°/1.2 | 同 | AlsTurnInPlaceSettings |
| RotateInPlace 参考视角转速映射 | 180–460°/s→1.15–3.0 | 同 | AlsRotateInPlaceSettings |
| 最大可行走坡度 | 50° | 50° | AlsCharacterMovement |
| Pivot 激活速度 | 200 cm/s | 2.0 m/s | PivotActivationSpeedThreshold |

## 5. 结论

核心运动栈（Grounded locomotion 全 gait/stance、Jump/Fall/Land、Roll、Stop distance
matching、Mantle、Overlay 叠加、Foot IK+pelvis、Ragdoll 进出、相机三模式）均已按 ALS
数值与状态语义复刻并通过各自 Gate；差异集中在"动画资产驱动的细节表现层"（GetUp
混合、TurnInPlace 图链、水平 foot lock）与性能优化空间，均为可增量增强项而非架构缺口。
