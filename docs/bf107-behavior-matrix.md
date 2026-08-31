# BF107 — P1 状态机语义与真实入口回归矩阵

本矩阵仅以 2026-08-31 当次重新读取的本地 ALS-Refactored 原生 C++、Cooked uasset 元数据和 Unreal MCP 结果为依据；不使用浏览器对话或外部审计结论作为实现事实。

## 当次原生事实摘要

- `AlsAnimationInstance::GetVelocity()` 将世界速度 inverse-rotate 到角色局部空间；`RefreshVelocityBlend()` 将归一化局部速度拆为 forward/backward/left/right，且对角分量按总绝对值归一化。`RefreshMovementDirection()` 在 ViewDirection/Aiming 中按视角相对 velocity yaw 选方向，但 VelocityDirection 或 Sprinting 时固定为 Forward。
- `AAlsCharacter::RefreshGait()` 分离 desired、max allowed 和 actual gait；actual gait 从实际 speed 阈值计算。Sprint 必须有 input、Standing，且受 aiming／rotation mode／相对 view yaw 50° 守卫。
- `ApplyDesiredStance()` 只在无 locomotion action 时在 Grounded 上请求 crouch/uncrouch；InAir 强制 uncrouch；实际 stance 只在 `OnStartCrouch()`／`OnEndCrouch()` 回写。Rolling 且设置允许时可请求 crouch。
- Jump 仅允许 Standing、无 locomotion action、Grounded。InAir 刷新分离 jumped、vertical velocity、ground prediction 与 air lean；ground prediction 仅在下降阈值以下、walkable sweep 命中且 curve gate 允许时产生值。
- Transition 由 `AllowTransitions` curve 全权重门控；动态 transition 比较 foot-lock target/lock distance，并延迟两个 frame。Rotate in place 仅在 Aiming 或 FirstPerson；Turn in place 仅在第三人称 ViewDirection、静止、yaw 条件持续满足时触发，并按 stance／左右／90°或180°选择资产，blend-out 使用 inertialization。
- Roll 仅在无 action 或同一 roll montage 已停止时启动；Grounded roll 以 input yaw 或 actor yaw 为目标并进入 Rolling action。Mantle 只在无 action、trace/space 检查通过时启动，锁定 movement 并创建单一 mantling root-motion source；停止时移除该 source、解锁 movement、恢复 walking。Ragdoll 启动时关闭 capsule、启用 mesh physics 并设置 Ragdolling action；停止时保存 final pose，按 ground 状态恢复 walking 或 falling，落地后以 GettingUp action 播放对应 get-up montage。
- `AB_Als` 的 MCP 资产元数据确认其父类为 `AlsAnimationInstance` 且实现 `ALI_Overlay::Overlay`。角色的 `SetOverlayMode()` 将 overlay tag 变更通知动画层；子状态的实际连线仍必须通过本项目运行日志证明。
- Unreal MCP 对 `A_Als_Sprint` 读取到 `Left`／`Right` sync markers、`PoseGait` curve、0.6s length、无 root motion；对 `A_Als_Roll` 读取到 1.5s length、启用 root motion；`AM_Als_Roll` 读取到 Foot IK／lock、layer 与 view curves。
- `AlsAnimNotify_SetGroundedEntryMode` 写 animation instance entry mode；`AlsAnimNotifyState_SetLocomotionAction` 在 begin 写 action、仅在 action 仍匹配时在 end 清空；`AlsAnimNotifyState_SetRootMotionScale` begin 设置 scale、end 在值仍匹配时恢复 1；`AlsAnimNotifyState_EarlyBlendOut` 在 input／locomotion／rotation／stance 条件命中时停止 montage。

## 回归矩阵

所有行的入口都是 `ALS_MANUAL_INTEGRATION_GATE=1 ALS_MANUAL_INTEGRATION_CASE=state_machine run_manual.bat`。日志路径均为该真实入口的标准输出，以列出的 `[MANUAL-INTEGRATION][P1]` 前缀可复查；在 BF107.Gate 前每行均保持未通过状态，不能以图加载、组件存在或截图替代。

| ID | 行为维度与原生语义 | 真实入口触发 | 可观测参数 | 预期状态路径 | 数值断言与日志路径 | 当前状态 |
| --- | --- | --- | --- | --- | --- | --- |
| P1-G01 | Standing Idle/Start/Cycle/Stop/Pivot；局部 velocity blend 驱动方向 | 原始 PlayerStart 上顺序施加静止、前进、反向、松开、再输入 | actual world/local velocity、F/B/L/R blend、standing outer/detail state、dominant weight、start/stop phase | Idle → Move → detail start/cycle → Stop → Idle；快速反向进入 pivot detail | 每个稳定窗口至少 12 frame；主分支权重 ≥ .5 且与局部 velocity 符号一致；`[P1][GROUND]` | 未通过 |
| P1-G02 | Standing/Crouching 的独立 locomotion 与 actual stance 回写 | 空旷地 crouch、移动、uncrouch；保留既有真实 canopy 路径 | desired/actual stance、collider half-height、grounded outer state、crouch transition、speed | Standing Idle/Move ↔ Crouching Idle/Move；阻挡时维持 actual crouch，释放后恢复 standing | desired/actual、capsule 和 graph state 连续记录；每次回切后 state/height 收敛；`[P1][STANCE]` | 未通过 |
| P1-G03 | Walk/Run/Sprint 的 desired/allowed/actual 分离与 sprint 守卫 | 八方向及 rotated-90 输入，含不允许 sprint 的 first-person/side/back 窗口 | desired/allowed/actual gait、input yaw、view yaw、actual speed、local direction、branch | Walk ↔ Run ↔ Sprint；被守卫拒绝时 Sprint request → allowed/actual Run | 每个 gait 取得至少一个稳定窗口；拒绝窗口 actual != Sprint；`[P1][GAIT]` | 未通过 |
| P1-A01 | Grounded/Jump/Fall/Land，按 grounded entry/vertical velocity 选择阶段 | 站立 jump hold，经过 apex 后落地 | locomotion state、jump variant、vertical velocity、ground prediction、grounded entry、air/land weights | Grounded → Jump → Fall → Land → Grounded | 状态顺序完整；air vertical velocity 穿过正负；land 后有限且 graph weight 收敛；`[P1][AIR]` | 未通过 |
| P1-R01 | Rotate/Turn in place 的 rotation mode、view yaw 与 stance 守卫 | 静止时分别驱动 third-person ViewDirection、Aiming、FirstPerson 的左右 look | rotation mode、view yaw/speed、turn/rotate state、turn play rate、body yaw | Idle → TurnLeft/TurnRight → Idle 或 Idle → RotateLeft/RotateRight → Idle | 左右 yaw 与选中方向同号；移动时 rotate/turn inactive；`[P1][TURN]` | 未通过 |
| P1-X01 | Roll action 的准入、yaw 目标与结束恢复 | Grounded move 后 roll；随后立即再次请求并等待结束 | locomotion action、roll active/slot weight、root-motion delta、stance、gait | Grounded → Rolling → Grounded | 第一次 action/slot 正常启动，重入请求不产生第二个 active action；结束后 action cleared；`[P1][ROLL]` | 未通过 |
| P1-X02 | Mantle 的 trace 准入、唯一 root-motion owner 与恢复 | 原始地形上的可见 mantle fixture，按既有路径触发 | mantle state、target/progress、root-motion owner/delta、locomotion action、movement lock | Grounded/Air → Mantling → Grounded | progress 单调并在有限 duration 内完成；同帧仅角色自身拥有 mantle root motion；`[P1][MANTLE]` | 未通过 |
| P1-X03 | Ragdoll/GetUp 的 physics 切换、final pose 与 grounded/air exit | 可见角色在原始 Playground 进入 ragdoll，grounded 后请求恢复 | locomotion action、ragdoll bodies、ground follow、joint drive、final pose/get-up action | Grounded → Ragdolling → GettingUp → Grounded | ragdoll 持续至少 12 frame；恢复后速度有限、ground follow 有效、action 最终清空；`[P1][RAGDOLL]` | 未通过 |
| P1-O01 | Overlay Relaxed/Ready/Aiming 与动作/移动中断 | Rifle overlay 依次请求 relaxed/ready/aiming，穿插移动与 action | overlay state、linked overlay FSM state、layer/aim weights、transition activity | Relaxed → Ready → Aiming → Ready/Relaxed；interrupt → Relaxed | 每个 state 有稳定窗口和有限 weight；中断后不残留 aiming/slot weight；`[P1][OVERLAY]` | 未通过 |
| P1-T01 | Transition priority、interrupt 与 inertialization | Turn/rotate/overlay transition 中反向输入或切换请求 | active transition、remaining time、source/target state、pose/weight delta、finite flags | active transition → permitted interruption → target or inertial recovery | 无 NaN/Inf；相邻 frame weight delta 有界；只消费一次对应 direct transition；`[P1][TRANSITION]` | 未通过 |
| P1-M01 | Root motion 的 clip/source 所有权 | Roll 与 mantle 分别运行，非动作时释放 | root-motion delta、owner entity、locomotion action、world displacement | Default → Roll/Mantle owner → None | owner 与角色 entity 一致；非动作窗口 delta 近零；`[P1][ROOT-MOTION]` | 未通过 |
| P1-M02 | Sprint `Left`/`Right` marker 的相位闭环 | 真实 sprint steady window | sync phase、marker name/order、leader/follower sample time、cycle duration | Left → Right → Left 循环 | marker 顺序正确；phase 有界且每角色独立；`[P1][SYNC]` | 未通过 |
| P1-N01 | Notify/Notify State begin/tick/end 生命周期 | Roll、mantle、sprint 依次播放 | event class/phase、action、root scale、entry mode、early-blend-out | event begin → active → end/recovery | 每一 state event 只配对一次 begin/end；end 后 action/root scale/early-blend flag 恢复；`[P1][NOTIFY]` | 未通过 |
| P1-I01 | 多角色状态、图时钟、root motion、notify 与 IK 隔离 | 在原始 PlayerStart 邻近可见位置生成第二角色并驱动相反行为 | 两 entity 的 all P1 telemetry、graph handle/clock、root motion、events、IK state | Character A path 与 Character B path 并行且互不改写 | entity id 不同；A 的 state/event/root motion/IK 写入不改变 B；`[P1][ISOLATION]` | 未通过 |

## 执行边界

- BF107.02 只实现 P1-G01 至 P1-R01 的真实入口 telemetry 与连续回归。
- BF107.03 只实现 P1-X01 至 P1-X03 和 P1-M01 的真实入口 telemetry 与连续回归。
- BF107.04 只实现 P1-O01、P1-T01、P1-M02、P1-N01、P1-I01 的真实入口 telemetry 与连续回归。
- 若任一条发现 P0 缺陷，先新增其前置 BF 条目、修复并重过对应 Gate；不得将失败掩盖为 BF107 通过。
