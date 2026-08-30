# bevy_als — 执行清单

> 本文件是唯一权威执行清单；执行纪律见 `agents.md`（仅规则），启动方式见 `goal.md`。
> 完成一项即勾选并一次提交：实现 → 自测 → 勾选 `[x]` → `git commit -m "<前缀><编号>: <条目摘要>"`（勾选与实现同一提交）。
> 基座模块提交前缀沿用 `M<编号>`；缺陷修复清单用 `BF<编号>`。
> 已归档清单：[archivedchecklist.md](docs/archive/archivedchecklist.md)

## BF104 — P0 局部移动方向、步态与最终 Locomotion Pose 闭环

目标：消除任意角色局部方向都由前向冲刺动画主导的问题。控制器的最终水平速度必须唯一地导出角色局部速度、方向混合和实际步态，并实际驱动正在求值的 locomotion pose；不得用输入意图、目标速度或孤立调试参数代替。此模块由审计报告触发，但每项开始时必须重新从本地 ALS-Refactored 提取当项事实，审计报告不作为实现依据。

- [x] **BF104.01 — D01：方向／步态源事实、现状链路与入口基线（P0）**
  - 依赖：BF103.Gate 已归档且已通过。
  - 执行前：从本地 ALS-Refactored 的角色移动、动画实例、Grounded locomotion 图／资产和相关曲线重新提取角色局部速度、direction、gait、sprint 守卫及方向资产选择的实际语义与数值；同时读取当前项目真实入口的完整数据流。
  - 验收：形成不含私有绝对路径的事实记录，逐段标明 source／项目／通用扩展归属；在原始 Playground、可见角色、正常相机和既有 grounded spawn 均通过后，以实际输入记录前后左右、四个对角和角色旋转后的 world velocity、local velocity、direction、desired／allowed／actual gait、active graph path、dominant clip／weight 基线。基线必须说明链路断点，不能以单帧姿态代替日志证据。
  - 当次 ALS 事实（2026-08-31，重新读取本地 ALS 插件 C++、`AB_Als_Standing.uasset` 二进制并以 Unreal MCP AssetTools 复核依赖）：`AAlsCharacter::RefreshLocomotion()` 读取实际 `GetVelocity()`，以水平 `Size2D()` 计算 speed，并仅在达到 1 cm/s 后更新 velocity yaw；`RefreshGait()` 分离 DesiredGait、MaxAllowedGait 与 Actual Gait，Actual Gait 在 walk/run 阈值各加 10 cm/s 后才切换。`CanSprint()` 要求有输入、Standing、aim 优先级允许；第三人称的 VelocityDirection／配置允许的 sprint-to-velocity 可直接许可，否则 input yaw 与 view yaw 的绝对差必须小于 50°。`UAlsAnimationInstance::RefreshMovementDirection()` 使用 world velocity yaw 减 view yaw；VelocityDirection 或 Actual Gait=Sprinting 时强制 Forward，其他模式按 forward half angle 70° 与 5° hysteresis 选 Forward／Right／Left／Backward。源 `AB_Als_Standing` 有 Forward、Backward、RightForward、RightBackward、LeftForward、LeftBackward 六个 WalkRun BlendSpace，均属 `Movement` SyncGroup、start `.2`；Sprint 与 SprintAcceleration 也属该组、start `.166667`，Acceleration 的 PlayRateBasis=`.833333`。这是 source 项目语义，不能用“始终按角色局部方向”替代其 view-direction／sprint 规则。
  - 项目现状（2026-08-31）：`refresh_gait_system` 已维护 `AlsDesiredGait`、`AlsMaxAllowedGait`、`Gait` 与连续 `AlsGaitAmount`；`direction_angle_driver`／standing layer 以 `LinearVelocity` 和 `AlsViewYaw` 计算方向，并在 Sprint／VelocityDirection 时选 Forward；standing layer 同时从角色 Transform 的 local horizontal velocity 生成四路 weight，写入 `als_standing_velocity_*` 与 `als_standing_direction_index`。但这些数据由多个层分别产生，现有 `anim_probe` 只读 `direction_angle`；graph state、主导 clip／weight、actual local velocity、三层 gait 与 standing pin 没有同帧联合记录，且现有 probe 持续输出多个未知 pin。因此现有静态图／组件与 Base PASS 都不能证明最终 pose 正确消费了当前事实。
  - 真实入口基线：在原始 Playground 的 PlayerStart、scene physics、可见角色和 grounded spawn 先后 PASS 后，`ALS_MANUAL_INTEGRATION_GATE=1 ALS_MANUAL_INTEGRATION_CASE=base RUST_LOG=info run_manual.bat`（`CARGO_BUILD_JOBS=2`）返回 0。运行日志记录 Running 时 `RightForward`（约 110°）、`Backward`（约 167°）、`LeftForward`（约 -70°）及 Sprinting／VelocityDirection 时 `Forward`／0°，但每帧仍为 `pins(speed=?,rate=?,...)`，没有 active graph path 或 dominant clip／weight；故仅可作为当前数据层基线，不能构成最终 locomotion pose 验收。D02 必须先收敛最终控制器事实与空间／命名合同，D04／D05 才可声明输出等价。
  - 归属：本项只记录事实和复现，不引入实现。D02 先评估通用 world-to-local／连续速度合同是否应归 `als-ext`；ALS 的 view/sprint 规则、参数键、GraphClip 资产选择和角色生命周期继续归 `als-core`／主项目。

- [x] **BF104.02 — D02：控制器最终速度至局部方向与连续混合参数（P0）**
  - 依赖：BF104.01。
  - 执行前：重新提取 D02 所覆盖的 ALS 方向计算、轴约定、死区／归一化及 Grounded 方向资产消费事实，并核对 Tnua 最终运动事实的公开接口。
  - 验收：只从控制器最终的水平 velocity 产生明确命名的 world、character-local、signed direction 和稳定 F/B/L/R（或等价连续）混合参数；角色朝向旋转后仍保持相同的局部含义，零速与阈值附近无方向跳变。可复用的坐标／速度桥接归属 `crates/als-ext`，ALS 参数映射留在项目／`als-core`；有单元和多角色隔离数值 Gate。
  - 当次 ALS／底座事实（2026-08-31，重新读取本地 ALS C++、`AB_Als_Standing.uasset` 二进制、本地锁定 bevy-tnua 源码与当前 lock）：ALS 的方向输入来自实际 `GetVelocity()` 的水平分量与 velocity yaw；其 animation instance 以 velocity world yaw 减 view yaw 选择方向，70°／5°区间后进入 Forward／Right／Left／Backward，而 Standing graph 的六个方向运动状态和四路 MultiWayBlend 共同消费方向资产。当前 lock 的 bevy-tnua 为 `9f831886647a4435b2d5894d4db9989f02854d6a`；其 pipeline 是 Sensors → UserControls → Logic → Motors。真实入口把 Tnua 放在 FixedUpdate、Avian physics 放在 FixedPostUpdate，因此 `LinearVelocity` 在 `PhysicsSystems::Last` 之后是本项目可用的最终物理速度，不能用输入向量或 Tnua desired motion 代替。
  - 归属决定：不含 ALS tag、资产或角色生命周期的水平 velocity → local forward/right、signed angle、归一化四路 weight 运算归 `crates/als-ext`；在 `PhysicsSystems::Last` 后快照最终速度、决定零速保留语义并将该统一事实供 direction／standing／overlay 消费的组件归主项目运行时层。视角方向、sprint 强制 Forward、ALS 参数键和动画状态选择保持项目／`als-core`，不污染通用扩展。
  - 补充事实与归属修正（2026-08-31，重新读取本地 `RefreshCrouchingMovement()`、`AB_Als_Crouching.uasset` 二进制与 MCP 依赖）：crouching 的 stride／play-rate 同样只读取 `LocomotionState.Speed`，其 Forward、Backward、LeftForward、LeftBackward、RightForward、RightBackward 六个 sequence player 也属于 `Movement` SyncGroup。因此 crouching 方向权重也必须消费同一最终速度快照。本仓库的 `als-ext` 已依赖 `als-core`，将 ALS 生命周期组件反向放入 `als-core` 会形成 Cargo 循环；通用数学仍留在 `als-ext`。D03 将 raw final velocity 收敛为 `als-core::AlsActualVelocity`，以供 core gait 与主项目共用；`AlsActualMotion` 仍只承担主项目的 local projection。这是经构建实证的依赖边界，而非修改 fork 或引入循环。
  - 实现：新增 `als-ext::HorizontalMotion`／`HorizontalVelocityBlend`，统一投影 world horizontal velocity 至角色 local forward/right，给出 signed radians 和 Manhattan-normalized Forward／Backward／Left／Right 权重。`als-core::AlsActualVelocity` 在 `FixedPostUpdate` 的 `PhysicsSystems::Last` 后快照 Avian `LinearVelocity`；主项目 `AlsActualMotionPlugin` 在这个共同事实与 ALS rotation response 之后产生 local 数据。standing、crouching、方向层和 root overlay 均改为消费该快照，方向最小速度由旧的 `.2 m/s` 对齐为原版 `1 cm/s = .01 m/s`。入口 `anim_probe` 同帧记录 world/local velocity、local angle 和四路 blend，零速明确归零；没有从 input 或 desired motion 生成动画方向。
  - 证据：`cargo test -p als-ext --lib motion --jobs 2` 通过（含 3 个新增通用数值测试）；`cargo test -p bevy_als --lib actual_motion --jobs 2` 通过（角色旋转、多角色隔离与停止归零）；`cargo test -p bevy_als --lib --jobs 2` 为 136/136，`cargo check --all-targets --jobs 2` 通过。`ALS_MANUAL_INTEGRATION_GATE=1 ALS_MANUAL_INTEGRATION_CASE=base RUST_LOG=info run_manual.bat`（`CARGO_BUILD_JOBS=2`）在原始 Playground startup Gate 后返回 0，并在移动、转向、sprint、crouching 与停止期间输出非输入推导的 `world_v`、`local_v`、`local_ang`、`blend` 与最终 `Base real_entry_paths -> PASS`。

- [x] **BF104.03 — D03：desired／allowed／actual gait 与 sprint 守卫（P0）**
  - 依赖：BF104.02。
  - 执行前：重新从本地 ALS-Refactored 提取 gait 选择、sprint 资格、stance／rotation mode／输入方向和实际速度的相关事实与阈值。
  - 验收：desired、allowed、actual gait 为独立可观测事实；sprint 仅在原版语义允许时成为 actual gait，侧移、后退、姿态／旋转模式不符或实际速度不符时不会持续取得前向 sprint 的最终所有权。所有切换、拒绝和恢复有逐帧数值 Gate，不依赖按键状态直接选择 pose。
  - 当次 ALS 事实（2026-08-31，重新读取本地 `AlsCharacter.cpp`／`AlsCharacterMovementComponent.cpp` 并以 Unreal MCP 读取 `CS_Als_Default`、`MS_Als_Normal`）：DesiredGait 初始 Running，仅表达请求；Grounded `RefreshGait()` 先算 MaxAllowedGait、写 movement component，再以实际 horizontal velocity 的速度和 walk/run 各 +10 cm/s 阈值写 Actual Gait。当前 `CS_Als_Default` 的 `bSprintHasPriorityOverAiming=true`、`bRotateToVelocityWhenSprinting=true`，故第三人称的有效 input 可许可 sprint 并转为 VelocityDirection；第一人称／不转向速度时仍须 input yaw 与 view yaw 小于 50°。`MS_Als_Normal` 的 direction-dependent speed 为 false，Velocity／View Standing 为 175/375/650 cm/s，Aiming Standing 为 165/375/650 cm/s，Crouching 为 150/200/300 cm/s，插值范围仍为 100°–125°。因此侧移／后退在默认第三人称 sprint 时应通过角色转向成为 source 定义的 Forward，不能错误地以“永远禁止横向 sprint”取代 source 行为。
  - 归属决定：Desired／allowed／actual gait、source 参数、input／view／stance／rotation guard 和 transition 事件属于 ALS 语义，保留 `als-core`；不含 ALS 规则的最终 world horizontal velocity 快照也应由 `als-core` 的物理适配层提供，供 core gait 与主项目 local-motion 层共同消费。`als-ext` 继续只提供无状态的空间／权重运算；这避免其既有 `als-ext → als-core` 依赖形成循环。
  - 实现：`AlsActualVelocity` 在 `FixedPostUpdate` 的 `PhysicsSystems::Last` 后写入水平 world velocity 与 speed，`refresh_gait_system` 只读取这个快照计算 threshold、GaitAmount 和 Actual Gait；主项目 local-motion snapshot 随后读取同一值。input request 与 `AlsStateRequest` 只写 DesiredGait，MaxAllowedGait 由 source guard 重算，Actual Gait 只由速度与允许层写入。`anim_probe` 同帧记录 desired／allowed／actual 及最终 velocity，`gait_contract_gate` 也改以最终 velocity component 驱动其断言。
  - 证据：`cargo test -p als-core --lib --jobs 2` 为 219 passed／2 ignored，包含 final-velocity gait、sprint permission 与 physical snapshot Gate；`cargo run --example gait_contract_gate --jobs 2` 输出 idle、1.84/1.86 m/s 阈值、第三人称横向 Sprinting、第一人称与 Air Running 的全量 PASS；`cargo test -p bevy_als --lib --jobs 2` 为 136/136，`cargo check --all-targets --jobs 2` 通过。真实入口以 `ALS_MANUAL_INTEGRATION_GATE=1 ALS_MANUAL_INTEGRATION_CASE=base RUST_LOG=info run_manual.bat`（`CARGO_BUILD_JOBS=2`）返回 0，在原始 Playground 中输出三层 gait 与最终速度，且 `Base real_entry_paths -> PASS`。

- [x] **BF104.04 — D04：方向参数接入实际 Graph 输出与动画选择（P0）**
  - 依赖：BF104.03。
  - 执行前：重新提取本项覆盖的 ALS Grounded locomotion、standing／crouching、walk／run／sprint 方向图、状态优先级和资产／曲线事实；核对当前实际 Graph 的播放路径。
  - 验收：D02／D03 的事实进入真实角色正在求值的 graph，而不是只留在 ECS、HUD 或未激活节点；前后左右与对角的 active FSM path、dominant clip／weight 随局部方向和 actual gait 改变，且后退／侧移不由前向 sprint 长期主导。通用图节点缺口优先归属 `als-ext`，ALS 资产选择留在项目；静态图存在不能替代实际播放日志。
  - 当次 ALS／图事实（2026-08-31，重新读取本地 `AlsAnimationInstance.cpp`／`AlsMath.cpp`、`AB_Als_Standing.uasset` 二进制和 Unreal MCP AssetTools 依赖）：source Standing 用 velocity yaw minus view yaw，VelocityDirection／Sprinting 强制 Forward，其他模式以 70° forward half-angle 与 5°阈值选择四向状态；其 `Movement States` 明确包含 Move Forward、Backward、RightForward、RightBackward、LeftForward、LeftBackward，并各自选择对应 WalkRun BlendSpace。六个 BlendSpace 都是 `Movement` SyncGroup，start `.2`；Sprint／SprintAcceleration 同组、start `.166667`，后者 PlayRateBasis `.833333`。这要求状态选择和四路权重都能被实际求值路径观察，而非从 writer 回显推断。
  - 当前项目链路（2026-08-31）：standing graph 已有六态 `InterruptibleFsmNode`、六个方向子图和 local F/B/L/R MultiWayBlend，节点链本身可到最终 root pose；但根 graph 对 `standing_state`／`movement_state`／`clip_index` 的输出仅接 `GlobalF32InputNode`，等同 writer 回显。`InterruptibleFsmNode` 的真实运行时状态保存在实际 `state_move` graph context，且该状态保存最后一次输出 pose；MultiWayBlend 直接读取四个 `als_standing_velocity_*` global input。因此不能以 root graph 的 writer echo 作为实际路径证据。
  - 归属决定：`InterruptibleFsmNode` 的 context state 与 last-output-pose 是既有通用能力，保持 `als-ext` 不变；Standing graph context 的只读遍历、六态标签映射和角色生命周期遥测均为 ALS 项目语义，留在主项目。主导 branch／weight 由实际 MultiWayBlend 已接线的四路 input 按同一正权重归一化规则计算，未给通用节点加入 ALS 专用常量，也无需修改依赖 fork。
  - 运行时缺口与处理（2026-08-31）：首次尝试把 telemetry output 逐层穿透 root graph 时，active transition graph 只定义 pose，额外 output 请求触发 `OutputMissing`；该试验已撤回。最终在 `AnimationGraphSet::Final` 后只读当前 `state_move` context 的实际 state、transition active 和已输出 pose，再读取当前 MultiWayBlend 消费的 F/B/L/R weights。这样不改 graph port，也不把 transition graph 的静态缺口误报为运行时选择。
  - 实现：`AlsStandingGraphTelemetry` 在角色上记录六态实际 FSM、transition active、`last_output_pose` 就绪和归一化 dominant branch／weight；`anim_probe` 同帧记录这些值以及实际 world/local velocity、三层 gait、direction。`manual_playground` Base 覆盖只有在 Standing 图产生 pose、FSM 正在稳定或过渡且 MultiWayBlend 权重有效时才标记 `standing_graph`，从而将运行时图事实纳入真实入口 verdict。
  - 证据：`cargo test -p bevy_als --lib --jobs 2` 为 137 passed；`cargo run --example standing_contract_gate --jobs 2` 返回 0；`cargo check --all-targets --jobs 2` 通过（仅既有示例警告）。真实入口以 `ALS_MANUAL_INTEGRATION_GATE=1 ALS_MANUAL_INTEGRATION_CASE=base RUST_LOG=info run_manual.bat`（`CARGO_BUILD_JOBS=2`）返回 0：日志先记录 transition `fsm_state=None transition_active=true pose_ready=true`，随后稳定窗口依次记录 `RightForward`／`Backward`／`LeftForward` 和 Sprinting 的 `Forward`，每项 `pose_ready=true` 且 dominant branch／weight 随局部速度变化；最终有 `EXECUTED standing_graph` 与 `Base real_entry_paths -> PASS`。

- [x] **BF104.05 — D05：真实入口八方向回归与可复查遥测（P0）**
  - 依赖：BF104.04。
  - 执行前：重新提取本场景所覆盖 ALS 行为的输入、速度、步态和方向输出事实。
  - 验收：`manual_playground` 在原始可见 Playground 地形、正常角色相机、既有 PlayerStart 与场景物理上自动执行前后左右、四个对角、旋转 90° 后的局部方向，以及 walk／run／sprint 允许与拒绝路径；每帧／每个稳定窗口记录实际速度、方向、三层 gait、graph path、dominant clip／weight。所有辅助物必须真实可见、落在原始地面且可清理；自动场景、HUD／probe 和日志路径可重复触发。
  - 当次 ALS／资产事实（2026-08-31，重新读取本地 `AlsAnimationInstance.cpp`、`AlsCharacter.cpp`、`AB_Als_Standing.uasset`，并用 Unreal MCP AssetTools 重取 `AB_Als_Standing` 依赖）：动画方向为 actual velocity yaw 减 view yaw；VelocityDirection 或 Actual Gait=Sprinting 固定 Forward。`CalculateMaxAllowedGait()` 仅在 Desired=Sprinting 时调用 `CanSprint()`；其要求有输入、Standing、aim 优先级允许，第三人称默认可因 rotate-to-velocity 许可，第一人称仍要求 input/view 夹角严格小于 50°；`CalculateActualGait()` 再按 walk/run 阈值各加 10 cm/s 落到实际 gait。uasset 的 Movement States 是六态有向图而非全连接：Forward 只能直接到 Backward／RightForward／LeftForward，Backward 只能直接到 Forward／RightBackward／LeftBackward，其他斜向边也有各自的 .5/.7/.75 秒 transition。MCP 依赖同时确认六个对应 WalkRun BlendSpace。
  - 缺陷与归属：真实入口第一轮使 velocity、direction 和 MultiWay weight 已改变但 FSM 保持旧 pose，暴露项目 `StandingMovementState::transition_index()` 只尝试 source→target 单边；例如 Forward→RightBackward 和 LeftBackward→Forward 没有直边，因而请求被丢弃。此图拓扑、状态路由和角色运行时行为属于 ALS 项目语义，留在主项目；未修改 `als-ext` 或依赖 fork。
  - 实现：`next_transition_toward()` 在本地已提取的六态直边上选择下一跳，先完成已发 transition 再继续多跳路线，保留每条源 transition 的 hips／feet 条件和时长。真实 `manual_playground` 新增 `DirectionRegression`：在可见原始地形中连续驱动 walk forward、run 的后/右/左/四对角、允许 Sprint、第一人称侧移拒绝 Sprint、同步旋转角色与视角 90°后的 forward；每个稳定窗口要求实际 world/local velocity 对齐、desired/allowed/actual gait、实际 FSM、pose ready 与 MultiWay dominant branch/weight 同时成立，否则 Base verdict 失败并列出 case。
  - 证据：`cargo test -p bevy_als --lib --jobs 2` 为 140 passed；`cargo run --example standing_contract_gate --jobs 2` 返回 0；`cargo check --all-targets --jobs 2` 通过（仅既有示例警告）。真实入口以 `ALS_MANUAL_INTEGRATION_GATE=1 ALS_MANUAL_INTEGRATION_CASE=base RUST_LOG=<manual integration filter> run_manual.bat`（`CARGO_BUILD_JOBS=2`）返回 0，逐项记录全部 11 个 case：walk forward、run 四基向与四对角、`Sprinting/Sprinting/Sprinting` 允许路径、第一人称横移 `Sprinting/Running/Running` 拒绝路径，以及 rotated-90 的 `world_align=1.000 local_align=1.000`；最终有 `EXECUTED direction_regression` 和 `Base real_entry_paths -> PASS`。

- [x] **BF104.Gate — 局部方向到最终 Locomotion Pose 全链路验收（P0）**
  - 依赖：BF104.01–BF104.05。
  - 验收：源事实、控制器最终事实、局部方向、步态权限、图参数、实际输出及真实入口八方向回归形成单一可追溯链；前、后、左、右、对角和角色旋转场景均以日志证明正确，且无输入意图／全局时间／默认前向 clip 的替代路径。
  - Gate 结果（2026-08-31）：通过。控制器链以最终物理水平速度计算 desired／allowed／actual gait，direction 以实际 velocity/view 规则选择，Standing runtime 再将非直连六态请求分解为源图允许的下一跳；图 context 证明实际 FSM 输出 pose，MultiWay 的实际输入证明 dominant branch／weight。真实 Playground 的八方向日志记录每个 case 的 world/local velocity、direction、三层 gait、FSM、branch／weight；没有未覆盖项或 graph error。
  - 可复查证据：`cargo test -p als-core --lib --jobs 2` 为 219 passed／2 ignored；`cargo test -p bevy_als --lib --jobs 2` 为 140 passed；`cargo run --example gait_contract_gate --jobs 2` 输出 idle、1.84/1.86 阈值、第三人称允许、第一人称拒绝和 Air 路径 PASS；`cargo run --example standing_contract_gate --jobs 2` 返回 0；`cargo check --all-targets --jobs 2` 通过（仅既有示例警告）。真实入口在原始可见 Playground、PlayerStart、scene physics、grounded spawn 全部 PASS 后，`ALS_MANUAL_INTEGRATION_GATE=1 ALS_MANUAL_INTEGRATION_CASE=base RUST_LOG=<manual integration filter> run_manual.bat` 返回 0，并输出 11 个方向 case、`EXECUTED direction_regression` 与 `Base real_entry_paths -> PASS`。

## BF105 — P0 蹲伏／起身的控制器、碰撞体与动画对称事务

目标：将 desired stance、控制器执行结果、headroom、collider、actual stance 与动画切换收敛为可重试的双向事务；阻挡起身不得丢失 desired standing。BF105 仅在 BF104.Gate 通过后开始。

- [x] **BF105.01 — C01：蹲起源事实、当前状态所有权与入口基线（P0）**
  - 依赖：BF104.Gate。
  - 验收：当前项开始时从本地 ALS-Refactored 重新提取 stance、角色移动、胶囊体、动画状态／转场及输入处理事实；记录 desired／actual stance、crouch action、headroom、collider 高度／中心、速度、动作打断和 animation state 的唯一写入者，并以空旷地、低顶棚和动作打断的真实入口日志定位不对称路径。
  - 当次 ALS／资产事实（2026-08-31，重新读取本地 `AlsCharacter.cpp`、`AlsAnimationInstance.cpp`、`AB_Als_Crouching.uasset`，并以 Unreal MCP AssetTools 重取 Crouching 图依赖）：DesiredStance 是独立目标；`SetDesiredStance()` 持久化目标并调用 `ApplyDesiredStance()`，grounded 且无 locomotion action 时请求 Crouch 或 UnCrouch，InAir 会请求 UnCrouch，Rolling 仅在 source setting 开启时可在开始时 crouch。真实 actual Stance 只在引擎 crouch 成功回调 `OnStartCrouch()`／`OnEndCrouch()` 后经 `SetStance()` 改写，后者同步 movement component；因此受阻 UnCrouch 不得先写 Standing。Crouching 图具有独立 crouch pose、六向 crouch walk、lean 与 crouch rotate 资产，stride 取 crouch curve，play rate 以 speed 除 animated crouch speed 与 stride 后限制到 2.0。
  - 项目当前所有权：输入 `crouch_pressed` 只创建 stance request；`process_stance_dimension()` 将已接受 request 写入 `AlsDesiredStance`，不直接改变附带 desired component 的 actual `Stance`。`capsule_reshape_system()` 是正常路径的 actual stance、`AlsCapsuleCrouched` 和 Avian capsule 唯一写入者：进入 crouch 同步写 crouched collider，站起先进行 standing capsule overlap，失败保留 desired standing、actual crouching 与 deferred 标记并逐帧重试，成功才恢复 standing collider 并写 actual。`locomotion_drive_system()` 仅在 actual crouching 时喂入 `TnuaBuiltinCrouch`；`tnua_config_sync_system()`、`stance_layer`、`crouching_layer`同样只读 actual `Stance`。组件缺失时 transition fallback 仍可直接写 actual，故 C02 必须将此正常角色路径收敛为显式事务并保留多角色隔离。
  - 真实入口基线：在原始 Playground 的 PlayerStart、scene physics、可见角色和 grounded spawn 全部通过后运行 Base。空旷开始后进入实际 Crouching，probe 连续记录 `cap_h=0.56`、crouching gait 与 Graph pose；原始场景内的低顶棚路径打印 `stand-up deferred: target capsule blocked`，随后在场景位置变化后打印 `clearance regained: standing capsule restored`，说明当前 collider retry 已实际发生。相同真实序列还记录 crouching 进入 Air 与后续 Roll/action 阶段，且现有 probe 未同帧输出 desired stance、Tnua action、collider 半高／中心及 crouching active graph path；这既是当前不可接受的证据缺口，也暴露了 source InAir UnCrouch 语义与项目动作链之间需要在 C02/C03 验证的差异。未把独立 `stance_gate` 的空白世界截图错误当作入口证据。
  - 证据：`ALS_MANUAL_INTEGRATION_GATE=1 ALS_MANUAL_INTEGRATION_CASE=base RUST_LOG=<motion and probe filter> run_manual.bat`（`CARGO_BUILD_JOBS=2`）返回 0，记录上述 Grounded Crouching、deferred、clearance regained、Air、Roll 和 `Base real_entry_paths -> PASS`；`cargo run --example stance_gate --jobs 2` 返回 0 但其 screenshot 目录缺失，故只作静态辅助检查而不计入真实入口验收。

- [x] **BF105.02 — C02：可拒绝、可重试的 collider 与控制器蹲起事务（P0）**
  - 依赖：BF105.01。
  - 验收：进入 crouch 与退出 crouch 成对修改／恢复 collider 尺寸和中心；headroom 拒绝时保持 desired standing 并在空间释放后自动重试；actual stance 只由成功的控制器和 collider 结果写入。通用事务／碰撞桥接归属 `als-ext`，ALS 规则及参数留在项目；空旷地连续 100 次蹲起和多角色隔离均有数值 Gate。
  - 当次 ALS／资产事实（2026-08-31，重新读取本地 `AlsCharacter.cpp`、`AlsAnimationInstance.cpp`、`AB_Als_Crouching.uasset` 并用 Unreal MCP 重取依赖）：DesiredStance 先持久化，再由 grounded 无 action 的 Crouch/UnCrouch 请求尝试变更；OnStartCrouch 和 OnEndCrouch 是 source actual stance 的唯一成功回调。Crouching animation 只消费 crouch pose／六向 crouch walk、stride 与受限 play rate，不能以 desired stance 提前替代实际物理结果。
  - 归属决定：`als-ext` 新增无 ALS 类型的 `CapsuleStanceTransaction`，表达 compact/expanded、blocked retry 与 revision，并以 100 次成对切换和实例隔离测试验证。因现有依赖为 `als-ext → als-core`，core 反向依赖 extension 会形成循环；故 ALS 适配层 `AlsCrouchTransaction` 保留在 core，将 `Stance`、Avian collider、Tnua controller 和角色生命周期映射到同一事务合同。该适配不含可复用的 ALS 外常量；没有修改依赖 fork。
  - 实现：所有 `AlsCharacter` 自动拥有 `AlsCrouchTransaction`。`capsule_reshape_system()` 在每次 desired/actual 不一致时同步 transaction，进入 crouch 只在写入 compact capsule 后提交 actual，站起仅在 standing capsule overlap 为空时恢复 collider 并提交 actual；被拒绝时保留 desired Standing、actual Crouching、compact dimensions 与 controller crouch，记录 `BlockedFromStanding` 和 retry count，随后每帧重新尝试。`anim_probe` 与真实入口输出 desired、actual、phase、retry、controller、half height、radius 和 headroom result；Base verdict 新增 `crouch_transaction` 覆盖。
  - 证据：`cargo test -p als-ext --lib crouch_transaction --jobs 2` 为 2 passed；`cargo test -p als-core --lib --jobs 2` 为 222 passed／1 ignored；core 数值 Gate 输出 `pairs=100 characters=2 revisions=[200, 200] -> PASS`，物理 Gate 输出 blocked desired Standing／actual Crouching／phase BlockedFromStanding／compact half=.56，并在移除阻挡后 restored actual Standing／half=.90。`cargo test -p bevy_als --lib --jobs 2` 为 140 passed，`cargo check --all-targets --jobs 2` 通过（仅既有示例警告）。真实 `manual_playground` Base 返回 0，实际记录 `desired=Standing,actual=Crouching,phase=BlockedFromStanding,controller=true,half=.56,radius=.30,blocked=true` 和递增 retry，最终 `EXECUTED crouch_transaction`／`Base real_entry_paths -> PASS`。

- [x] **BF105.03 — C03：实际 stance 驱动动画与中断恢复（P0）**
  - 依赖：BF105.02。
  - 验收：动画只读取 actual stance，双向转场可完成或被明确中断并恢复；蹲伏移动、jump、roll、mantle、落地和动作取消后，desired standing 仍可完成为 actual standing。每个状态变化输出 transaction、controller、collider、animation path 的联合证据。
  - 当次 ALS／资产事实（2026-08-31，重新读取本地 ALS C++、`AB_Als.uasset` 二进制并用 Unreal MCP 重取 `AB_Als`／`AB_Als_Grounded` 依赖）：`SetDesiredStance()` 先持久化 desired，再调用 `ApplyDesiredStance()`；无 locomotion action 时 Grounded 调用 Crouch／UnCrouch，InAir 调用 UnCrouch。actual Stance 只在 `OnStartCrouch()`／`OnEndCrouch()` 的成功回调经 `SetStance()` 改写。locomotion action 变化会重新调用 `ApplyDesiredStance()`；Rolling 且 source setting `bCrouchOnStart`（默认 true）时调用 Crouch。`AB_Als` 的 Grounded linked graph 有 Standing Input、Crouching Input 和 `Grounded States`；其 Crouching 状态与 transition 的实际输出 pose 必须从 `InterruptibleFsmState` context 读取，不能由 writer global 回显推断。`RefreshCrouchingMovement()` 仍只以 actual locomotion speed 计算 stride／play rate，并将 play rate 限制至 2.0。
  - 归属决定：desired/actual stance、Air／action 语义、collider、Tnua action、Grounded graph 选择与角色生命周期均为 ALS 项目语义，保留 `als-core`／主项目；不新增 ALS 常量至 `als-ext`，不修改依赖 fork。`AlsMantleState` active 时 project adapter 保持 existing actual capsule，action 结束后再按 desired 恢复；现有 roll drive 继续按 source default 在开始请求 Crouching、结束恢复 start stance。
  - 实现：空中 stance request 现在立即更新 `AlsDesiredStance`，而不在 request slot 中排队；`capsule_reshape_system()` 在无 action 的 Air 将 target 固定为 Standing，但保留 desired Crouching 和 `forced_standing` transaction evidence，落地后才实际 compact capsule／Crouching。Mantling active 时不提前改变 actual stance。`AlsGroundedGraphTelemetry` 在 `FixedPostUpdate` 的实际 graph Final 后读取 Grounded FSM context，输出 actual state、transition active 与 pose ready；probe 记录该 path 与 forced state。真实 Base 增加一个在原始 PlayerStart、正常相机和可见 Playground 中执行的短时 Air sequence：先稳定 grounded，临时禁用 mantle 起跳以隔离普通 jump，实际观察 Air desired Crouching／actual Standing，落地 Crouching FSM，再请求 Standing 并观察 Standing FSM；随后继续既有 Turn、Mantle、Platform、Roll。方向回归的 rotated-90 case 改按实际 local velocity blend 判定 branch，并将每 case 稳定窗口扩为 3 秒，避免把转身期间的 writer 假设作为 pose verdict。
  - 证据：`cargo test -p als-core --lib --jobs 2` 为 224 passed／1 ignored，包含空中 desired Crouch 保持 standing collider、落地 compact capsule 与 action target 约束；`cargo test -p bevy_als --lib --jobs 2` 为 142 passed；`cargo run --example grounded_contract_gate --jobs 2` 输出 18/18 source direct topology 与 linked PASS；`cargo check --all-targets --jobs 2` 通过（仅既有 examples warnings）。真实入口以 `ALS_MANUAL_INTEGRATION_GATE=1 ALS_MANUAL_INTEGRATION_CASE=base RUST_LOG=<manual integration filter> run_manual.bat` 返回 0：先记录 `CROUCH-AIR-REQUEST`，随后 `desired=Crouching actual=Standing forced_standing=true`；落地记录 actual Crouching、Grounded FSM state=2、controller=true，恢复记录 `desired=Standing actual=Standing state=1 controller=false`；最终所有 Base coverage（含 direction、mantle、platform、roll）均为 `EXECUTED` 且 `Base real_entry_paths -> PASS`。

- [x] **BF105.04 — C04：真实入口蹲起回归与低顶棚释放场景（P0）**
  - 依赖：BF105.03。
  - 验收：`manual_playground` 只在原始 Playground 地面添加可见、可渲染且带 collider 的短时低顶棚摆件，自动验证空旷 100 次蹲起、低顶棚拒绝、离开后的自动起身、蹲伏移动及动作中断恢复；入口日志同时证明角色／相机／地形持续真实可试玩。
  - 当次 ALS／引擎事实（2026-08-31，重新读取本地 ALS C++、ALS movement component 与本地 UE 5.8 `CharacterMovementComponent`）：ALS movement component 启用 `bCrouchMaintainsBaseLocation=true`。source `Crouch()` 在缩小 capsule 后沿 gravity direction 下移中心以保持底部；`UnCrouch()` 用略膨胀的 standing capsule 在候选 standing location 做 blocking overlap，保持底部时先将中心上移 half-height 差，若仍 encroached 则直接 return，不调用 `SetIsCrouched(false)`／`OnEndCrouch()`。仅通过后才提交 capsule size、位置与 actual stance 回调。ALS character 在动作／locomotion mode 变化时重试 desired stance；这些事实要求 project 的 headroom 测试在 standing target center 而非旧 crouched center 上进行。
  - 归属决定：base-preserving capsule center、target overlap、Avian `Position` 同步和实际 transaction 是 ALS character adapter 语义，保留 `als-core`；不把 Stance、ALS parameters 或 Playground fixture 下沉到 `als-ext`。短时 canopy 仅属于真实入口验收：复用项目已有 mesh/material，在原始 PlayerStart 地面生成静态 render mesh + collider，完成或 timeout 都经 entity lifecycle 清理，不替代任何原场景地面。
  - 实现：`capsule_reshape_system()` 以当前 actual capsule base 推导 target center；Crouch 下移中心、standing clearance 在上移后的 standing center 测试，成功时同帧写 Transform、Avian Position、collider、transaction，失败时保留 crouched center。核心物理 Gate 新增 base 不变性与 blocked/recovered transaction center 断言。真实 Base 在 C03 后新增 `CrouchClearance`：在 PlayerStart／正常相机／已加载 Playground 下先完成 100 个 actual Crouching→Standing pairs；角色已 crouched 后才生成红色短时 canopy（2.4m×0.16m mesh、同尺寸 collider），请求 standing 观察 blocked retry，crouch 移出 canopy 水平范围后让原 transaction 自动恢复 standing，再清理 canopy，继续 Turn、Mantle、Platform、Roll。
  - 证据：`cargo test -p als-core --lib --jobs 2` 为 225 passed／1 ignored，包含 `stance_capsule_centers_preserve_the_support_base` 和 physical headroom blocked/recovered center Gate；`cargo test -p bevy_als --lib --jobs 2` 为 142 passed；`cargo check --all-targets --jobs 2` 通过（仅既有 examples warnings）。真实 `manual_playground` Base 返回 0，记录 `CROUCH-CYCLES pairs=25/50/75/100`、`CROUCH-CANOPY center=... bottom=...`、`CROUCH-CANOPY-BLOCKED desired=Standing actual=Crouching retries>=2 half=.560 state=2 controller=true`、非零 `CROUCH-MOVING`、`CROUCH-CANOPY-RECOVERED desired=Standing actual=Standing retries=0 half=.900 state=1` 以及 `CROUCH-CANOPY cleaned entity=...`；随后全部 Base coverage、Mantle、Platform、Roll 均 `EXECUTED`，最终 `Base real_entry_paths -> PASS`。

- [ ] **BF105.Gate — 蹲伏／起身事务全链路验收（P0）**
  - 依赖：BF105.01–BF105.04。
  - 验收：源事实、input request、controller result、headroom、collider、actual stance、graph state 和真实入口行为日志完全对应；任何拒绝、重试或动作打断都不会造成永久 crouch 锁死。

## BF106 — P0 Foot IK 的空间合同、两骨求解与真实入口调度

目标：重建 foot target 从 world 至 armature-local、parent-local、bone-local 的显式合同，并将两骨 IK、权重／门控和层级传播固定在动画 pose 求值之后、skinning 之前。BF106 仅在 BF105.Gate 通过后开始。

- [ ] **BF106.01 — I01：Foot IK 源事实、坐标约定与当前调度基线（P0）**
  - 依赖：BF105.Gate。
  - 验收：当前项开始时从本地 ALS-Refactored 重新提取 ControlRig、foot trace、骨骼／虚骨、曲线、动画门控和执行顺序；记录无私有路径的空间、单位、reference rotation、足部和 knee plane 合同，并以真实入口四向旋转、坡面／台阶边缘、IK 开关输出目标空间、长度误差和 pole sign。

- [ ] **BF106.02 — I02：显式空间转换、足部 trace 与无命中恢复（P0）**
  - 依赖：BF106.01。
  - 验收：世界 hit position／normal 不会直接写入 bone-local transform；以类型或明确字段区分各空间，统一转换 up／forward 轴与 reference pose，并在无命中时平滑回零且不保留陈旧 target。通用空间／骨架桥接归属 `als-ext`，ALS trace 参数和资产映射留在项目；数值 Gate 覆盖旋转不变性和多角色隔离。

- [ ] **BF106.03 — I03：稳定两骨 IK、镜像 pole 与动画后调度（P0）**
  - 依赖：BF106.02。
  - 验收：左右腿使用镜像且稳定的 pole plane，近共线、过伸、零长度与权重切换都有退化保护；IK 固定在动画 pose 后、层级传播前执行，且不会被后续系统覆盖或重复叠加。关闭 IK 的基础动画保持正常，0↔1 权重连续；通用 solver／调度归属 `als-ext`，ALS 门控留在项目。

- [ ] **BF106.04 — I04：真实入口坡面、台阶边缘与动作门控回归（P0）**
  - 依赖：BF106.03。
  - 验收：`manual_playground` 在原始可见地形上以短时可见、可渲染且带 collider 的坡面／台阶摆件验证四向旋转、台阶边缘、crouch、jump、roll、mantle、land 与 IK 开关；逐帧记录所有目标空间、hip／knee／ankle 误差、pole sign、IK weight 与执行阶段，不得以静态截图代替。

- [ ] **BF106.Gate — Foot IK 空间、求解与调度全链路验收（P0）**
  - 依赖：BF106.01–BF106.04。
  - 验收：源事实、空间转换、trace、两骨 solver、权重／门控、系统顺序和真实入口日志构成闭环；四向旋转、坡面、台阶边缘、蹲伏与落地均无翻膝、扭曲、单帧跳变或跨帧发散，关闭 IK 后基础动画正常。

## BF107 — P1 状态机语义与持续真实入口行为回归（待 BF106.Gate）

目标：在三个 P0 闭环通过后，以真实运行日志逐项验证 Idle／Start／Cycle／Stop／Pivot、standing／crouching、walk／run／sprint、grounded／jump／fall／land、turn／rotate、roll／mantle／ragdoll／get-up、overlay、转场中断／惯性化、root motion、sync marker、Notify State 与多角色隔离。

- [ ] **BF107.01 — S01：建立按行为维度的源事实与真实入口回归矩阵（P1）**
  - 依赖：BF106.Gate。
  - 验收：当前项开始时重新提取本地 ALS-Refactored 的每个覆盖维度事实；为每个行为登记真实入口触发、可观测参数、预期状态路径、数值断言和日志路径。未覆盖项必须明确列为未通过，不能用图加载、组件存在或单帧截图代替。

- [ ] **BF107.Gate — P1 行为矩阵与持续回归验收（P1）**
  - 依赖：BF107.01 及其后续按矩阵展开的条目。
  - 验收：全部登记行为均有真实入口连续运行证据；若矩阵发现新增缺陷，先将其登记为按依赖顺序的 BF 条目并修复、重新过相关 P0／P1 Gate。

## 背景与基础事实（已核实，持续有效）

- 本机 bevy 源码：`0.19.1`（release-0.19.1）。`bevy_animation_graph` 0.11.0 与 `bevy-tnua` 均基于 Bevy 0.19，兼容。
- bevy_animation_graph 已有节点：ClipNode、ChainNode、BlendNode（含骨骼 mask/Difference/Additive）、BlendSpace1D/2D、布尔运算节点等；图资产 RON 序列化、支持编辑器热重载；Pose 以 `BoneId` 路径表达；RootMotionMode 提取已可用（M8.8）。曲线/ramp/距离匹配等由 als-ext 经公开扩展点补齐。
- bevy-tnua 已有 basis：Walk（coyote time）、Jump（高度/悬挂控制）、Crouch、Dash、Climb、Knockback、WallSlide；物理后端 avian3d 0.12.1 / rapier3d 0.17.0。
- ALS-Refactored 关键结构：4 模块（ALS / ALSCamera / ALSExtras / ALSEditor）；状态体系用 GameplayTags（非枚举）；自定义 AnimNode_CurvesBlend；ControlRig 输入结构 `AlsControlRigInput`；IK 相关骨骼 `foot_l/foot_r`、`ik_foot_l/ik_foot_r`、`VB foot_l/VB foot_r`；关键曲线名 `LayerHandLeft/Right`、`HandLeftIk/HandRightIk`、`FootLeftIk/FootLeftLock/FootRightIk/...`；动画资产分组：Grounded / Air / Actions / Transitions / TurnInPlace / RotateInPlace / Overlays / View / Base。
- 本地环境路径、引擎版本等隐私信息见 `docs/env.md`（已 gitignore）。

## 模块依赖顺序

```
M0 环境与资产管线 ─► M1 ECS核心状态模型 ─► M2 曲线基础设施 ─► M3 tnua运动基座
─► M4 动画图基座 ─► M5 Locomotion状态机动画 ─► M6 Overlay/Aim ─► M7 FootIK/CR等价物
─► M8 特殊动作(跳跃翻滚) ─► M9 Ragdoll ─► M10 相机 ─► M11 调试与总验收
─► BF103 Notify 事件管线（已归档） ─► BF104 方向／步态闭环 ─► BF105 蹲起事务
─► BF106 Foot IK 闭环 ─► BF107 状态机语义与持续回归
```

## 附录 A：与 ALS 的刻意差异（非缺陷）

| ALS | bevy_als | 理由 |
|---|---|---|
| AnimBP 状态机 | 显式 system 路由 + animation graph RON | ECS 数据流，无隐藏图执行 |
| GameplayTags | 普通 Rust 枚举组件 | 编译期检查 |
| ControlRig | 分析式 IK systems | 无运行时 rig 编译 |
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
- 音频（Metasound）、Niagara 特效、ALSCamera 动画实例仅保留挂钩点。
