# BF134 移植差异台账

本轮起始版本 `9105cdf`；对标本地 ALS-Refactored 与 UE 5.8 的当次 source。严重级与任务边界见 `CHECKLIST.md` BF134。仅登记有源事实、可达 target consumer 与可重复触发条件的 P2 及以上差异；旧结论不代替 fresh 审计。

当次覆盖 inventory：146 个源 C++/header（ALS 112、ALSCamera 12、ALSEditor 16、ALSExtras 6），181 个 target Rust，15 个 installed AnimBP root、359 个生成依赖文件和 15 份安装 manifest，文件缺失 0。源 revision 为 ALS `b754d6f0f2bb03741d301f8fb88077ebfe561e17`、UE `396c9f059903aed5fec78ecd3d437a40c6415368`；完整相对路径和 SHA-256 在本地证据 `target/bf134/inventory.json`。B_Als_Character 原生 reader 39.050 秒成功；watchdog self-test 通过。Editor/AI/网络与刻意差异按执行清单排除。

## 覆盖

| 移植闭包 | 本次 source/target 核对 | 尚未核销边界 |
|---|---|---|
| Character/CMC、状态、输入、出生、运动基座 | 实际源码位置与入口清单正在核对 | 算法/配置/坐标/生命周期 |
| Roll/Mantle/Ragdoll/GetUp/Stop 与 montage | 待执行 | 动作门槛、时钟、根运动、退出与重建 |
| AnimBP/FSM、cached/linked Pose、BlendSpace、曲线与同步 | 待执行 | 全递归 source 与当前 runtime consumer |
| Notifies、脚步与贴花 | 待执行 | 消息区间、过滤、所属实例、实际 shader |
| Control Rig、骨架与 IK | 待执行 | 源 RigVM/坐标/当帧 Pose/生命周期 |
| PhysicsAsset、关节、CCD、时钟与回读 | 待执行 | 实际求解器配置与 consumer |
| ALSCamera、相机动画、shake 与 trace | 待执行 | 实例、时钟、坐标、碰撞与 view |
| Overlay、Extras 场景/材质/UI | 待执行 | 资产/装配/输入与显示的源合同 |

## 已确认差异

### BF134.01A — P2：地面刹车低速未归零

- 源事实：当次本地 UE `UCharacterMovementComponent::ApplyVelocityBraking` 在非零刹车后将不超过 10 cm/s 的速度归零；仅摩擦时仍按源长度平方的小数阈值归零。无刹车/摩擦和小于 MIN_TICK_TIME 的 Delta 不触发归零。ALS 的 CalcVelocity 调用该源函数。
- Target：`grounded_velocity_step_with_braking_friction` 仅限制本次减速度，不应用源停止阈值；`grounded_dynamics_motor_system` 与 stop-distance prediction 直接消费，松开输入的低速尾帧仍移动。
- 漏检：已有刹车 Gate 检查 3.75–4 m/s 与超速转向，未检查源 0.1 m/s 停止边界及实际 motor。
- 反例：实际 motor 在 0.08 m/s、0.001 秒、源非零刹车下仅降低 0.015 m/s，仍输出 0.065 m/s；定向 Rust 测试稳定失败（exit 101）。修复后 15 项 dynamics 回归通过；阈值来自当次原生角色和 UE 源常量，经既有离线工具生成，其他已生成 movement 常量 diff 为零。
- 修复：只消费离线生成的源停止阈值和 MIN_TICK_TIME，按源处理无刹车/纯摩擦/超速保速，并让模拟循环处理精确 minimum Tick。实际 motor、0/微小/精确 Tick、停止门槛两侧、纯摩擦/零刹车和超速控制通过；精确 Tick 旧值 `(0.08, 0 step)`、源期望 `(0, 1 step)` 的二次漏检先 red 后修复。
- 消融/静态：移除停止阈值与 minimum Delta 条件分别稳定 red，逐字节/mtime 恢复；15 项最终回归通过。fresh 全量 15 根/39 source/374 产物完整 diff=0，345 当前节点规格与全部递归 source closure 通过；原始导出未改写。
- 真实入口：`target_game/play` 最终版 146.507 秒构建；公共 startup、Grounded 15/15、StandingStop 10/10 功能 Gate 均通过，graph/primary failure/fault/temporal 异常为零。证据：`target/animation_visual/bf134/01a/{startup,grounded,standing_stop}`。截图关闭，完整性能的 max_eval_p95_ms 失败保留，不宣称性能验收。
- 状态：已修复并验收。工具/profile/摘要 schema 的中途失败已保留，不计为语义反例；报告仅提取判定/覆盖，逐帧 trace 留在原始证据。

### BF134.01B — P2：鼠标 Look 的源单位缩放错误

- 源事实：当次 B_Als_Character sensitivity 上下/左右均为 1；IMC_Als_Default 的 Mouse2D 映射无 modifier，IA_Als_LookMouse 为 Axis2D、无 modifier/trigger。EnhancedPlayerInput 直接消费 KeyState.RawValue，项目 LegacyInputScales=False；角色 Input_OnLookMouse 将值交给 AddYaw/PitchInput 以 degrees 累加。
- Target：真实 Binding::mouse_motion 经 Action/bridge 原样发布，但 view 多乘 0.0025 radians/count；同样 12 个水平 counts，目标只转 0.03 radians，源为 12 degrees（约 0.20944 radians）。旧 mixed-units 测试用同一个目标常量重算期望，遮蔽差异。
- 反例/修复：真实 MouseMotion→EnhancedInput Action→bridge→view 测试先稳定 red，再删除额外倍率，只保留标准 degree/radian 转换。11 input/6 view 回归通过；鼠标不乘 Delta，rate 输入仍按源 240/90 degrees/s 乘 Delta，下一帧清零控制通过。原场景已按相同转换常量反算注入 counts，无须添加适配层、状态或分支。
- 真实入口：最终正常优化 `target_game/play` 构建 145.414 秒；startup、Camera 11/11、Head 14/14、StandingTurn 11/11 功能 Gate 通过，graph/primary/fault/temporal 异常为零。证据：`target/animation_visual/bf134/01b/{startup,camera,head,standing_turn}`。截图关闭，完整 max_eval_p95_ms 失败仍保留。
- 状态：已修复、验收并单独提交。图、生成器和资产未改，BF134.01A 的全量生成树证据继续有效；manifest 首次空 graph 的 schema 失败已纠正并通过，不修改验证器。

### BF134.01C — P2：输入累加与平值映射优先级

fresh Move/Look 均为 TakeHighestAbsoluteValue。UE 普通映射保持资产顺序，逐轴 `>=` 接受后者；Bevy 默认 Cumulative 把相反键归零，其公开 MaxAbs 用 `<` 保留前者。真实 W+S 反例旧值 `(0,0)`、源期望 `(0,-1)` 稳定 red。使用现有 ActionSettings::MaxAbs 并反向排列 source 对应的绑定，13 项真实输入回归通过，包括所有相反键、两轴组合、键盘/手柄强弱竞争和平值；鼠标、普通按键、清零/Shift/trigger 控制保留。移除 accumulation、恢复旧绑定顺序分别 11 pass/2 semantic fail，byte/mtime 恢复后13 green；没有新 evaluator、消息、状态缓存或依赖修改。首次把 Bundle 当 SpawnableList、测试多余 Into 两编译错误，以及只读 stderr 而漏掉 stdout panic 的摘要错误均已纠正，不计新语义反例。

最终同启动器构建 146.414 秒，startup/Interface 15/15/Grounded 15/15 公共功能 Gate 通过，图/primary/fault/temporal 异常全零；证据 `target/animation_visual/bf134/01c/{startup,interface,grounded}`。源码 hashes、root ABI/layout/editor/资产边界通过，图/资产/converter 未变；完整生成 diff=0 证据仍有效。状态：已修复并单独提交；截图/完整性能原暂停范围保留，max_eval_p95_ms 失败未核销。

### BF134.01D — P2：rate Look 使用缩放时钟

fresh IA_Als_Look 使用 ScaleByDeltaTime、禁止暂停触发；UE EnhancedPlayerInput 将当前游戏 Delta 除以 effective dilation 后传给 modifier，零 dilation 以 RealTimeDeltaSeconds 回退。Target view 使用游戏 Delta，导致0.15 Slomo时同样真实输入角度只有源0.15倍。真实 Action→bridge→view 反例先red，再按源完整公式消费现有 Virtual/Real；直接用Real的初版会绕过有效帧clamp，追加失败反例后删除该简化。暂停时不消费Look/Mouse（source两Action均禁止暂停触发），零速未暂停仍沿源回退。普通/0.15/2/0速、clamp、鼠标独立与暂停控制通过；unscale/pause/zero-dilation三消融分别red，byte/mtime恢复后14 input/6 view/Turn驱动夹具green。无新clock/cache/state/依赖，两个旧caller迁移真实资源/期望，look_input_gate同入口profile编译通过。

最终target_game/play预构建144.005秒通过，startup/Camera 11/11/Head 14/14/AnimationClock 7/7/StandingTurn 11/11公共功能Gate均通过，ready=true、graph/primary/fault/temporal异常全零；证据 `target/animation_visual/bf134/01d/{startup,camera,head,animation_clock,standing_turn}`。source hashes、root ABI/layout/editor/资产边界通过；图/资产/转换器未变，全量生成diff=0仍有效。状态：已修复并验收，单独提交；截图关闭，完整性能max_eval_p95_ms失败未核销。临时批处理误用空LASTEXITCODE造成未执行场景，已按逐份success报告核对并补齐，不能把批处理退出0当全场景通过。

### BF134.01E — P2：view pitch 范围和绕回与原生 CameraManager 不符

当次原生Controller的PlayerCameraManagerClass=None、父类为PlayerController，12图/179节点无pitch/manager/modifier覆盖；UE SpawnPlayerCameraManager回退原生类。当次native CDO的ViewPitchMin/Max为±89.900002度（源89.9f的导出值），ProcessViewRotation经ClampAngle归一化到(-180,180]再限制对称范围。Target常量±85且直接线性clamp，实际87度被截为85，大幅鼠标输入的绕回方向也错误。旧单元测试从同一个target常量重算期望，200度错误方向亦被断言为正确。

实际MouseMotion→Action→view先在87度稳定red；仅修正范围后-180度再次red，源为+89.9而旧target为-89.9。离线工具从两份fresh原生记录生成原字段，现有clamp复用标准库余数和两条源绕回分支，无新配置module/资源/state/cache。15 input/6 view通过，含范围两侧、±180、±200、整圈与多圈及鼠标下一帧清零。实际函数的独立标准库fixture在移除源值、remainder或任一绕回分支时分别语义失败，源码未变；源字段唯一性消融亦稳定拒绝重复缺口。source hashes、root ABI/layout/editor/boundary与常量check通过，图/图转换器/共享规则/资产未变。

最终target_game/play预构建156.307秒成功，公共startup（15.727秒）、Camera 11/11（20.292秒）、Head 14/14（22.093秒）功能Gate均通过，ready=true、graph/primary/fault/temporal异常零；证据 `target/animation_visual/bf134/01e/{startup,camera,head}`。状态：已修复并验收、单独提交。五份focused fixture的不同hash与实际source hash未变已核对，实际入口单独验证；原始记录不变，15/39/374完整生成diff=0证据继续有效。截图关闭，完整性能max_eval_p95_ms失败保留，未核销性能验收。

## 待反例核对的源差异

当前执行清单 BF134.01F–H 已登记 CMC/braking 子步、输入有效性/阻塞和 gamepad 二维 dead-zone 归属。它们已有具体 source-target 链，尚未通过本轮最小反例与修复 Gate，不计作已完成项。其余模块仍按覆盖表继续核对；本轮五项完成不代表全范围目标已完成。
