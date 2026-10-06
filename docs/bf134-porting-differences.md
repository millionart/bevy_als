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

BF134.01G1/G2a/G2b/G2c/G2d1/G2d2/G2d3/G2d4已修复并验收；G2其他consumer/H2及剩余Action仍登记角色输入有效性/阻塞和gamepad二维dead-zone归属，未提前核销。当前已修复十六项P2移植差异，F1-G是工具源等价误报，单独记录且不计移植差异。其余模块仍按覆盖表继续核对，全范围目标未完成。

### BF134.01G1 — P2：动画输入绕过已消费状态

当次 ALS ConsumeInputVector 在 bInputBlocked 时返回零；Character.RefreshInput 从实际 CMC acceleration 获取并归一化 InputDirection，再生成 bHasInput 和保留世界方向。AnimationInstance.RefreshLocomotionOnGameThread 直接复制角色状态，不重新读取按键。fresh角色原生读取82.632秒成功，相关三份CPP当次快照与SHA已保存。

Target locomotion_drive 已清零阻塞输入，AlsActualMotion 已记录该实际basis及停止保向，provider却用raw command/override重新判定并计算方向；同一帧角色无输入而动画有输入，可达生成FSM/locomotion/head consumer。实际loaded角色的按键Y+阻塞，经motor、快照与writer后，快照has_input=false、动画true，最小测试92.415秒稳定exit101。旧测试只验证raw helper/override，漏掉消费阶段。已复用现有has_input/input_yaw_degrees，删除两个raw helper、相关query字段和player重复保向分支；G2的角色/CMC门槛与其他consumer独立待核对。

候选14项provider回归通过（84.597秒），新的实际角色夹具保留阻塞前后的输入方向、override清零/细小值和世界角度；第一次45度检查因相机producer覆盖手工PlayerView而失败，已设置真正的view输入后重跑，不把夹具误设当source差异。分别删除消费标志和保留方向，两次实际consumer消融81.817/81.780秒均稳定exit101，三个不同source hash及最终byte/mtime恢复已核对；没有新增状态、helper、缓存或适配层。最终14项恢复回归81.357秒通过，最终manifest source hashes/root ABI/layout/editor/runtime asset boundary通过（2.397秒）。

当前启动器target_game/play正常优化预构建150.885秒成功，公共startup15.799秒、Grounded15/15（42.562秒）、StandingResponse6/6（20.011秒）功能Gate全部通过，ready=true、graph/primary/fault/temporal零，证据 `target/animation_visual/bf134/01g1/{startup,grounded,standing_response}`。状态：已修复并验收，G1单独提交，parent G及全范围继续；S1实际00:56:12/偏差−19分48秒，S2实际01:04/偏差−12分钟。原始记录、图和共享转换规则不变，完整15/39/374生成diff=0仍有效。截图关闭及完整max_eval_p95_ms失败保留，不宣称性能验收。

### BF134.01G2a — P2：着地制动读取了错误帧的输入

fresh角色原生读取37.562秒，CharMoveComp的bTickBeforeOwner=True；当次UE MovementComponent注册owner Tick对CMC的prerequisite。ALS着地通知在CMC内部读LocomotionState.bHasInput选factor0.5/3，之后owner RefreshInput才更新该状态。Target landing系统却重读当帧raw/override并用平方1e-6门槛，上一轮ActualMotion.has_input已有角色消费结果。着地同时松开输入、初速3.75m/s/50ms，实际landing→motor输出1.3062501m/s，源应保留factor0.5输出2.775m/s，最小反例80.132秒稳定exit101。

BF102.18已有限复开；旧Gate只给window helper直接传bool，未验证输入来源/阶段。候选已删除raw query及额外门槛，复用既有快照，没有新状态/helper/adapter；20项dynamics回归78.650秒通过，八组前帧状态/当前按键/override/细小输入边界及实际motor、旧窗口/预测与F1/F2/base/air控制覆盖。source hashes/root ABI/layout/editor/runtime-boundary通过（2.420秒）。状态：已修复并验收，G2a独立提交；G2其他consumer及全范围未核销。

消融移除/强制保留输入77.995/78.823秒，分别在源速度/源factor断言稳定exit101，三个不同hash与byte/mtime恢复已核对。恢复后20项运动回归77.941秒、真实motor→snapshot→writer集成1.365秒通过；该状态的actual producer亦覆盖。S1开始01:12、预估01:37、实际01:26:12、偏差−10分48秒成功；最终target_game/play正常优化预构建149.405秒成功；startup15.558秒、Grounded15/15（42.860秒）与原variable UphillJump6/6（36.289秒）全部功能Gate通过，ready=true、graph/primary/fault/temporal零，证据 `target/animation_visual/bf134/01g2a/{startup,grounded,uphill_jump}`。S2开始01:26、预估01:41、实际01:33、偏差−8分钟成功，BF102.18有限复开已重新核销。图/共享转换/资产和原始记录未变，已有15/39/374完整生成diff=0有效；截图关闭、完整max_eval_p95_ms失败保留，不核销性能或其他输入consumer。


### BF134.01G2b — P2：步态及派生旋转模式绕过owner阶段

fresh角色37.136秒、TickBeforeOwner=True，当次ALS setter只保存DesiredGait；CMC先移动，owner随后RefreshInput/Locomotion→RefreshGait→RefreshRotationMode/Rotation，速度上限供下一次movement消费。target gait/aim却在Update motor前刷新，Running→Walking切换首帧实际(Walking,3.125m/s)、源应(Running,3.75m/s)，数值反例80.710秒稳定red。当前未消费输入false/raw Y时目标Sprint、源Running，独立实际system反例0.296秒red；两者指向同一owner状态归属，不能只放宽raw输入门槛。

候选复用ActualMotion的已消费标志与世界方向，把gait和派生mode移到physics/snapshot之后、响应之前；mode直接写入并发布既有合法StateTransitionEvent，删除request转发及跨schedule伪依赖，无新cache/set/第二次通用处理。actual两帧motor84.218秒、69项定向回归通过，source/static2.416秒通过。首次Query tuple超上限36.207秒编译失败已修正；随后80.859秒schedule cycle由已存在的旧动画曲线准备→rotation链暴露，fresh GetCurveValue证明不能反转该链，保留原链并删除多余provider直连。状态：已修复并验收，G2b单独提交；原始记录/图/转换规则未变，rotation输入门槛与底层归一化仍待核对，parent G/G2未提前核销。


G2b最终验收：四actual-source消融88.105/83.304/85.352/86.535秒均语义red，三文件byte/mtime恢复确认；两条冗余调度边分别删除后最终同一69项回归82.392秒通过，算法不增加cache/新set/第二次通用处理。最终source/static2.394秒与正常target_game/play构建149.370秒通过；startup15.693秒、Grounded15/15（42.342秒）及Sprint8/8（17.388秒）功能Gate全部通过，ready=true、graph/primary/fault/temporal零，证据 `target/animation_visual/bf134/01g2b/{startup,grounded,sprint}`。S1实际02:04:33/偏差−16分27秒，S2实际02:17/偏差−7分钟成功。已有15/39/374全量生成diff=0有效，截图关闭与完整max_eval_p95_ms失败保留，不宣称性能或全范围完成。


### BF134.01G2c — P2：旋转重新判定raw输入，低速有效输入误停转

fresh角色37.554秒、Character与Vector.h证明SetInputDirection先GetSafeNormal，再按角色bHasInput/bHasVelocity生成moving，旋转读取该状态。target却重取raw/override/player-view并按length²>f32 EPS判定；0.0002有效输入与0.01m/s边界实际actor yaw=0，源应进入moving并转向，consumer最小反例83.835秒稳定red。BF102.11已有限复开，旧单位输入测试未覆盖消费门槛/来源。

候选直接读取已有ActualMotion.has_input/input_yaw_degrees，删除重复raw判断/方向重建与query。32项rotation回归86.436秒通过，fixture明确提供源消费状态，无运行时fallback/cache/new helper；source/static2.375秒通过。状态：已修复并验收，G2c独立提交，BF102.11有限输入消费边界复验关闭。source唯一blocked=true调用在同一函数进入GettingUp，未证明独立default P2边界，不把这个API差异当已修复项；原始资产/图/共享转换未改，其他normalization/motor边界继续审计。


G2c最终验收：消费标志/世界角度两actual-source消融81.678/81.501秒均语义red，三hash及byte/mtime恢复确认；32 rotation+10 gait+5 aim+21 dynamics+1 gait-layer+1完整schedule共70项回归green（driver90.096秒）。当前正常target_game/play构建154.958秒成功；startup15.560秒、Grounded15/15（42.704秒）、StandingResponse6/6（19.962秒）公共功能Gate全通过，ready=true、graph/primary/fault/temporal零，证据 `target/animation_visual/bf134/01g2c/{startup,grounded,standing_response}`。S1实际02:38:42/偏差−26分18秒、S2实际02:46/偏差−12分钟成功；原始记录/图/转换规则未改，完整15/39/374生成diff=0有效。截图关闭与完整max_eval_p95_ms失败保留，不宣称性能或全范围完成；底层输入normalization/motor及二维dead-zone后续继续。


### BF134.01G2d1 — P2：drive额外归零微小非零输入，误触发制动

fresh角色37.016秒及当次UE CMC/Vector/Math源闭合：ScaleInputAcceleration仅clamp最大长度，ComputeAnalog对非零加速度算modifier，CalcVelocity用Acceleration.IsZero判制动；GetSafeNormal的unitless门槛属于角色locomotion flag。Target drive却在move_len≤1e-4时清零。actual drive→motor在输入1e-5、初速0.2m/s、50ms时旧desired/velocity均0，源应0.20001m/s，反例82.432秒稳定red；旧正常/0.01模拟量Gate未覆盖微小非零输入与已存在速度。

候选删除drive截断分支，真实反例80.960秒green且角色has_input保持false。新的独立source常量提取器生成既有Motion文件中的UE_SMALL_NUMBER unitless tolerance，保持物理非零输入与角色flag归属分离；原始记录不改，没有修改已有graph converter或共享转换规则。self-test/生成0.304/0.266秒、源门槛两侧和保向fixture78.945秒、source/static2.387秒通过。drive-cutoff/owner-tolerance两actual-source消融86.017/81.563秒均语义red，两文件byte/mtime恢复确认；最终79项回归driver90.842秒green，含actual drive→motor→snapshot→writer和完整调度。

状态：已修复并验收，G2d1独立提交；当前入口正常构建154.790秒成功，startup16.568秒、Grounded15/15（43.471秒）、StandingStop10/10（26.573秒）功能Gate全部通过，ready=true、graph/primary/fault/temporal零，证据 `target/animation_visual/bf134/01g2d1/{startup,grounded,standing_stop}`。S2开始03:12、预估03:32、实际03:22、偏差−10分钟成功；既有converter/共享规则与原始记录未改，完整15/39/374生成diff=0有效。截图关闭与完整max_eval_p95_ms失败保留，不宣称性能或总目标完成。S1开始02:52、预估03:17、实际03:11:50、偏差−5分10秒成功；其他motor<=1e-6/物理方向安全归一化按G2d2继续，不能把角色flag容差重新用作加速度归零门槛。


### BF134.01G2d2 — P2：motor把非零加速度误判为零，物理方向容差归属错误

fresh角色49.715秒与当次CMC/Vector/Math：Acceleration.IsZero决定刹车，AccelDir对实际厘米加速度GetSafeNormal，输入增量仍用原Acceleration，overspeed回保检查Acceleration·OldVelocity。target按desired_amount≤1e-6截断且归一化输入，微小输入驱动错误刹车；actual amount1e-7/初速0.2/50ms旧0和诊断false、源0.2000001，86.394秒稳定red。MaxAcceleration0+command1旧0.2、源0，独立native binary0.289秒red。

候选先构造实际加速度、用零向量选源分支、以既有源基转换的physical EPS=1e-12归一化friction方向并保留raw加速度增量，诊断同步；原unitless输出不变。24 dynamics修正回归83.384秒通过，旧friction夹具的acc0错误期望已按源分支改为acc1与独立数值(2.7,0,-0.32)，不放宽反例。四关键分支消融均语义red，额外reverse guard删除后green并裁去，最终81项回归driver90.533秒通过。状态：已修复并验收、G2d2单独提交。全量15根/39当次source/374完整新旧产物diff added/removed/changed=0（1510.609秒），同次全部递归closure48.057秒PASS；未安装candidate或复开无影响的其它图。当前正常target_game/play构建155.080秒成功，startup15.505秒、Grounded15/15（42.338秒）、StandingStop10/10（26.232秒）功能Gate全部通过，ready=true、graph/primary/fault/temporal零，证据 `target/animation_visual/bf134/01g2d2/{startup,grounded,standing_stop}`。S1开始03:28、预估04:03、实际04:04:09、偏差+1分9秒；S2开始04:04、预估04:24、实际04:16、偏差−8分钟，均成功。原始record未改，截图关闭及完整max_eval_p95_ms、grounded max_frame_ms性能失败保留，不宣称性能或总目标完成；min-analog floor相邻事实独立待定级。


### BF134.01G2d3 — P2：min-analog floor错误地被MaxSpeed封顶

fresh角色39.208秒与当次CMC GetMinAnalogSpeed/CalcVelocity、公开BlueprintReadWrite/ClampMin=0速度设置闭合。源MaxInputSpeed=max(MaxSpeed*Analog,MinAnalog)，target增加min(minAnalog,MaxSpeed)。合法WalkSpeed0.1m/s/MinAnalog0.25m/s/amount0.01/初速0.2/50ms，actual motor旧0.100000024、源0.21，92.041秒稳定red；默认较高速度会隐藏该合法配置故障。

仅删除无源封顶，actual motor与六floor边界/25 dynamics86.827秒green；恢复旧cap消融79.223秒语义red，byte/mtime/hash恢复确认。最终82项恢复回归driver87.002秒、source/static2.394秒通过，没有新状态/抽象/分支或资产修改。S1开始04:24、预估04:49、实际04:37:38、偏差−11分22秒成功。状态：已修复并验收、G2d3独立提交；当前正常target_game/play构建148.427秒成功，startup15.541秒、Grounded15/15（43.108秒）、StandingStop10/10（26.827秒）公共功能Gate全通过，ready=true、graph/primary/fault/temporal零，证据 `target/animation_visual/bf134/01g2d3/{startup,grounded,standing_stop}`。S2开始04:38、预估04:58、实际04:49、偏差−9分钟成功。图/转换规则/原始记录未变，15/39/374完整生成diff0有效；截图关闭和完整max_eval_p95_ms失败保留，不宣称性能或总目标完成。


### BF134.01G2d4 — P2：模拟量modifier遗漏MaxAcceleration门槛

fresh角色37.548秒、当次CMC ComputeAnalogInputModifier与公开CurveVector配置：MaxAccel>UE_SMALL_NUMBER且加速度非零才计算modifier，否则modifier0；Acceleration.IsZero与输入增量保持独立。目标将非零加速度直接等价于modifier有值，合法MaxAcceleration1e-11m/s²/初速1.5/50ms的actual motor旧1.125、源应0.75，85.135秒稳定red。

候选补按0.01转换的1e-10 scalar门槛，诊断复用同一规则，无新cache/helper/状态；26 dynamics85.888秒、门槛两侧/zero/diagnostic81.337秒与最终83项回归driver90.106秒通过。去scalar/恢复raw诊断两消融77.994/77.162秒均语义red、byte/mtime/hash恢复，source/static2.644秒通过。扩展提取分支后已重新全量15根/39 source/374完整新旧产物diff0（1485.621秒），同次全部递归closure50.020秒通过；原始record未改且candidate未安装。S1开始04:56、预估05:41、实际末项Gate05:56:32、偏差+15分32秒成功，协调处理延迟如实记录。状态：已修复并验收，G2d4单独提交；当前正常target_game/play构建152.028秒通过，startup16.053秒、Grounded15/15（43.155秒）、StandingStop10/10（27.002秒）公共功能Gate全部通过，ready=true、graph/primary/fault/temporal零，证据 `target/animation_visual/bf134/01g2d4/{startup,grounded,standing_stop}`。S2开始06:02、预估06:22、实际末项Gate06:19:19、偏差−2分41秒成功，协调处理滞后如实保留。原始record未改，candidate未安装，截图关闭与完整max_eval_p95_ms失败保留，不宣称性能或总目标完成。

### BF134.01H — P2：摇杆径向死区被逐轴执行

- Source：当次原生IMC_Als_Default的Gamepad_Left2D/Right2D分别使用DeadZone_0/_2，均Type=Radial、LowerThreshold=0.2、UpperThreshold=1.0。UE InputModifiers.cpp先计算Size2D和GetSafeNormal2D，再将重映射长度乘回方向；动作累加在mapping modifier之后。
- Target/触发：spawn_als_actions把Axial preset的两个独立binding各自附加DeadZone；斜向(0.15,0.15)被逐轴归零，而源输出(0.010723304,0.010723304)。中等斜向(0.5,0.5)源每轴0.4482233，旧目标0.375；非均匀斜向也改变方向。普通手柄移动和视角可达，定级P2。
- 最小修复：通过公开InputModifier把两个已有binding先聚合为私有二维group，再复用插件内置DeadZone，转发处理后的mapping值给原Move/Look；不添加自定义死区算法、输入资源、逐帧缓存或依赖fork。键盘继续在mapping之后按源MaxAbs与优先级竞争。插件Binding没有二维摇杆类型且InputReader为私有，不能直接在单个binding读取另一轴；合并死区到整个Action会改变组合键与混合输入，须由消融确认。
- 验证/状态：actual Action→bridge最小red106.845秒成立，32项恢复回归通过，两设计消融均为预期语义失败；单轴、死区内、径向边界、饱和、正负斜向、mixed keyboard、原鼠标/时钟/暂停合同闭合。H-G修复后36恢复回归再次覆盖输入/视角/步态；Input代码hash保持一致，新startup16.282秒、原Grounded15/15（42.926秒）、新Head14/14（20.847秒）功能通过，graph/primary/fault/temporal零。source/static8.187秒通过，H已修复验收并单独提交；原Head工具失败保留，Y方向候选另留H2。图/转换规则/原始资产未改，全15/39/374转换diff=0证据继续有效；截图关闭、max_eval_p95_ms失败保留，仅功能验收。证据target/bf134/01h。

### BF134.01H-G — Head验收驱动意图遗漏，不计移植差异

- 原始公共Head报告前三pitch窗口通过，case3实际速度1.75、has-input=true但velocity-direction=false，工具等待该分支后超时。源公式无错误，graph/primary/fault/temporal均零；保留该失败。
- 当次B_Als_Character原生默认DesiredRotationMode和RotationMode均ViewDirection；C++ SetDesiredRotationMode保存意图，owner RefreshRotationMode据此派生actual。工具仅写actual请求会被正确的owner刷新覆盖，G2b生产生命周期无源差异证据，不复开生产算法。
- 最小实际driver→input_request→owner测试稳定red；工具在窗口入口走既有rotation-mode input边沿改变保存意图，删除无效actual请求，无新状态/helper/schedule。source/static、移除press消融和36恢复回归通过；新生产构建167.434秒，startup16.282秒/Head14/14（20.847秒）功能通过，graph/primary/fault/temporal零。原失败保留，不改阈值，工具子项单独提交；截图关闭、max_eval_p95_ms延后失败保留。证据target/bf134/01h_gate。
