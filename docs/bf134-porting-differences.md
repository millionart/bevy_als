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

### BF134.01F1 — P2：CMC尾段时间步及braking预算耦合

fresh角色/MS_Normal/normal CV确认源最大时间步.05秒、8次预算、braking_substep≈1/33，低gait源curve为2000cm/s² acceleration、friction5。当次ALS PhysWalking与UE PhysFalling递增Iterations后调用原生GetSimulationTimeStep：只在remaining>max且非末轮时取min(max,remaining/2)，末轮消费剩余量，最后按MIN_TICK_TIME保底。Target取min(max,remaining)，还在无输入且非零braking factor时用制动步长降低CMC上限，额外强制max_step≥1e-4/iterations≥1。75ms的实际motor旧50+25ms、源37.5+37.5ms，旧最终速度(0.984375,0,1.9251981)、源(0.9902344,0,1.8872353)稳定red；factor3使旧outer步数3、源2的另一反例亦red。旧测试只数3步，没有约束每步时长或实际最终速度。

现有单循环删除braking耦合与额外下限，消费源half-tail/末轮/最低Tick；没有新helper/module/state/cache/依赖。17项dynamics回归通过，含实际grounded/air motor、停止边界、预测、基座和roll恢复。实际outer函数的step-recording focused fixture分别移除half-tail、末轮消费、最低Tick、原生零预算及恢复braking耦合，五项均语义red，control通过，实际源码未改；该夹具只验证步长传递，实际motor单独验证。原有全部movement常量以当次native source重生成到临时文件后逐字一致，source hashes/root ABI/layout/editor/boundary通过；图/转换器/共享规则/资产未改，全量生成diff=0仍有效。状态：已修复，source/static与四个公共功能Gate通过，单独提交；F2内部braking与stop预测独立积分算法未在本项核销。

真实入口最初variable UphillJump触发calf_r时序Gate，已按下方G项确认源同样运动并修复工具分类；原失败及fixed控制保留。最新同target_game/play正常优化构建145.665秒、同源码时效补查16.639秒成功；startup15.423秒、原variable UphillJump6/6（33.590秒）、Grounded15/15（42.311秒）、FootIk22/22（80.233秒）功能Gate全部通过，graph/primary/fault/temporal零。独立工具G先提交、CMC外层后提交；图/共享转换/资产未改，15/39/374全量生成diff=0仍有效，原始source不可变。截图关闭，完整max_eval_p95_ms失败保留，未核销性能或F2内部制动。

### BF134.01F1-G — 验收工具误报，不计作移植差异

fresh CR_Als原生读取41.912秒，79 initial global的position/rotation/scale按完整坐标基与program逐项一致，translation误差0。当次RigUnit_TwoBoneIKSimple→ControlRigMathLibrary→AnimationCore公式复算保存三帧，source local calf delta1.300658994rad、target1.3006219rad，world quaternion最大角误差1.88e-5rad；两个相邻目标距离由.57456m增至.81941m，腿接近源总长度.82769m。源同样产生约87rad/s的运动，不能把它当移植姿态缺陷或补偿，旧100ms source IK测试未覆盖该边界。

最小实际输入fixture先red，再用已有相邻输入/program初始数据/独立basic solver，只有源预期local delta和实际hip/calf世界rotation同时匹配时分类为源运动。仅作用于calf的local rate，其他subject、jump/plane flip、位置/scale/端点和错误输出保留原检查；所有阈值及pose代码未改。错输出/错历史/错delta/其他骨骼/全flip反例通过；移除输出核对、相邻delta核对分别稳定red，byte/mtime恢复后12 temporal+17 dynamics green。额外NaN疑点探针原样green，撤回测试且源码hash与29项恢复完全一致，没有新增guard。

source/static/ABI/layout/editor/boundary通过；正常优化target_game/play最终构建145.665秒及时效补查16.639秒成功。最终startup15.423秒、原variable UphillJump6/6（33.590秒）、Grounded15/15（42.311秒）、FootIk22/22（80.233秒）功能Gate全部通过，graph/primary/fault/temporal异常零。variable自然重跑未触发SOURCE-MOTION，不声称再命中原帧；原33.227秒失败与fixed控制、源三帧证明及fixture独立保留。状态：工具验收完成、单独提交，F1代码随后按其独立Gate提交；完整性能max_eval_p95_ms失败保留，截图关闭。G2相对14:42偏差+21分31秒，时间纪律漏检和纠正记录在清单，未抹除失败或沿用旧窗口。


### BF134.01F2 — P2：缺少独立制动子步，着地制动过强

当次UE ApplyVelocityBraking在非零摩擦时用clamp(braking_substep_time,1/75,1/20)与remaining/2分步；RevAccel从本次初始速度一次确定，每步防反向，全部内部步后才应用源停止阈值。fresh角色39.849秒、normal CV35.720秒确认配置≈1/33、ground friction供braking使用、gait2=2000/1250cm/s²与friction4；ALS落地factor3已有源规则。Target仅一次friction/减速度更新，原外层曾错误地用braking步长模拟内层，F1删除该混合后未消除内部算法差异。

actual motor在50ms、3.75m/s、factor3、CMC预算1时旧.875m/s、源1.30625m/s，稳定red；源内部为25+25ms，不受CMC预算限制。向原helper传现有配置并inline唯一源循环，19项回归通过，含三个clamp/default值、无输入、超速正反/正交同帧输入、zero-friction一次处理、防反向、停止门槛、F1/air/base/预测/roll控制。配置、half-tail、zero-friction和防反向四actual-block focused消融分别red，control通过，实际源码未变。独立正交期望数字一度抄错导致18pass/1fail，已按源向量归一化公式复算纠正，不把此失败当新移植缺陷；无新state/helper/module/适配层/图或转换规则。

source/static/ABI/layout/editor/boundary闭合；target_game/play最终正常优化构建156.028秒成功，startup（16.116秒）、Grounded15/15（42.257秒）、StandingStop10/10（26.758秒）、原variable UphillJump6/6（32.539秒）功能Gate全部通过，graph/primary/fault/temporal零。状态：已修复并验收，F2独立提交、父F核销；图/共享转换/原始记录未改，全量生成diff=0有效。S1数小时超时与估时漏检已复盘，第二轮按1.5倍窗口20:14闭合，S2实际20:36:47/偏差+1分47秒完成。完整max_eval_p95_ms失败及uphill max_frame_ms失败保留，截图关闭，不核销性能或全范围。


## 待反例核对的源差异

BF134.01G/H仍登记输入有效性/阻塞和gamepad二维dead-zone归属，尚未通过本轮修复Gate；不计已完成。当前已修复七项P2移植差异，F1-G是工具源等价误报，单独记录且不计移植差异。其余模块仍按覆盖表继续核对，全范围目标未完成。
