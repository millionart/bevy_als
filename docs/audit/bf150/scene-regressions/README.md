# BF150.04 Playground 场景回归

以 `65cf6c59` 在同一正式 Playground 入口的完整报告为对照。原验收为墙钟 p95 ≤ 25 ms、最大 ≤ 75 ms、图求值 p95 ≤ 4 ms、每渲染帧最多 3 个物理步，以及 40 m/s 的骨骼相对位置速率门槛。所有运行使用 `run_manual.ps1` 的 `target_game/play`。 [起点报告](../extension-ownership/README.md)

## Default Overlay

原生 Default 图以显式帧序列 `0,1,2,1,0` 求值。`A_Als_Idle` 使用 LocalSpace Additive，基姿态为 Stand Pose 的第 0 帧。目标运行时已经使用源准备序列。原验收观察器从 glTF 重新采样比较两处序列求值器，并把已经烘焙的 Idle 差值再做一次差。真实加载角色的最小用例在旧代码稳定失败。观察器改用运行时同一采样入口后，11 项定向测试通过，六个动作窗口达成。首轮完整判定仍记录图求值 p95 4.309 ms 和 Idle 的静止输入。 [原生读数](overlay-native.json) [旧行为反例](overlay-loaded-before.watchdog.json) [定向回归](overlay-focused.watchdog.json) [正式动作与预算](overlay-scenario/runner.json)

`AB_Als_Standing` 的 Idle 直接读取 `A_Als_Stand_Pose` 第 0 帧。Overlay 验收继续检查 Layering 的 Locomotion 输入存在并含有效骨骼，在 Idle 窗要求 Secondary Motion、加性结果和分层输出产生原幅度的运动。其余窗继续检查 Locomotion 的运动。 [Idle 原生节点](standing-idle-native.json) [旧动作窗误判](data-reuse-default-overlay/validator.log.gz)

正式复验通过六个动作窗、姿态链和完整预算。墙钟 p95／峰值为 19.346／40.332 ms，图求值 p95 为 3.699 ms。 [正式复验](selective-default-overlay/runner.json)

## Footprint

`FES_Als_Default` 的投射贴花使用 `SpawnAttachedToTraceHitComponent`。UE 通知以命中组件、网格倍率和足部旋转建立挂接，材质域是 `MD_DeferredDecal`，纹理为 `T_Als_Footprint_M`。首个零贴花控制的两个 Bevy 接收器均已加载并在真实视口。读回头为 `[652,1,0,1]`，代表数据长度、启用标记、样本数和请求号。缓冲 ID 在请求期间保持一致，片元入口诊断仍无写入。恢复旧 GPU 实例构建的对照得到 36 请求、576 样本及全部动作覆盖，墙钟 p95／最大为 39.279／108.111 ms，图求值 p95 为 4.572 ms。 [原生读数](footprint-native.json) [CPU 请求诊断](cache-cpu-footprint/decal_render.json) [GPU 实例对照](foot-gpu-instances/runner.json)

Bevy 0.20.1 按预处理模式分配批次，CPU 实例写入要求 `DynamicUniforms`。仅挂 `NoIndirectDrawing` 会选择 `PreprocessingOnly`，正式进程会丢弃 CPU 的实物批次并输出错误。选用 CPU 构建时同时把渲染队列配置为 `GpuPreprocessingMode::None`，同一入口通过 36 请求、576 样本和全部动作。此时第一帧进入 Footprint 视野新增了六条 tile 管线，渲染线程在第 86 帧用时 78.768 ms。 [CPU 路径回归](cpu-batching-footprint/runner.json) [管线日志](codegen-footprint/process.stderr.log.gz)

UE 在静态网格组件上预缓存材质管线。目标的真实地砖材质参与初次渲染的管线准备后，准备窗的同一帧只编译四条剩余管线，用时 52.813 ms。墙钟峰值由 93.996 ms 降到 62.286 ms。随后共用数据优化的完整 Footprint 达到墙钟 p95／峰值 18.382／62.034 ms、图 p95 3.234 ms。 [地砖管线对照](receiver-pso-footprint/process.stderr.log.gz) [完整 Footprint](data-reuse-footprint/runner.json)

移除临时探针的正式复验通过全部动作、36 个请求和 576 个 GPU 样本。墙钟 p95／峰值为 19.012／62.298 ms，图求值 p95 为 3.776 ms，每渲染帧最多 1 个物理步。 [正式复验](combined-footprint/runner.json)

## Control Rig

两份起点报告里，双脚终姿速度与传入 IK 的目标位移相符。新增现场诊断以 3.357 ms 的现步复现左脚 57.427 m/s，该步前一动画步为 48.849 ms，墙钟间隔为 9.132 ms。此时足锁权重为零。UE `RefreshFeetOnGameThread` 从上一已发布骨架读取脚部 socket。原生不均匀步长对照对 49 个运行帧的 98 个足部目标复算，上一姿态配当前组件变换的最大误差为 0.000000716 cm，当帧姿态的中位差为 3.060 cm。UE 主循环从 `FPlatformTime::Seconds` 计算游戏 delta，Bevy 0.20.1 的 Automatic 默认优先采用渲染结束时戳。 [原生请求](rig-socket-clock.request.json) [原生读数](rig-socket-clock.native.json.gz) [正式诊断](cache-control-rig/report.json.gz) [旧时钟反例](main-clock-before.watchdog.json)

Playground 让 Automatic 使用主线程时间。手动步长、源世界时间限幅和速率断言按原配置执行。原生的 24／3.081 ms 交替输入在两个不同时间点确实产生 41.740 和 46.888 m/s 的左右足速。正式普通时钟下，两次 Control Rig 诊断均为零骨骼速率告警。 [时钟复算](rig-socket-clock-comparison.json) [正式 CPU 路径](cpu-batching-control-rig/runner.json)

## 图求值

真实进程采样中，Rig 内部 p95 0.210 ms。`run_animation_player` 的栈覆盖姿态缓存、分层及同步组解析。按 UE CachedPose 更新与取值阶段拆除预热中的无用副本后，Control Rig／Default Overlay 图计时仍为 5.275／4.929 ms，已撤销该试改。采样会扰动帧时，预算由无进程采样的正式运行判定。 [热栈](control-rig.sample.txt.gz) [预热消融](cache-prewarm.patch.gz) [Control Rig 结果](cache-control-rig/runner.json) [Overlay 结果](cache-default-overlay/runner.json)

## 复跑

```powershell
foreach ($case in 'startup', 'control_rig', 'default_overlay', 'footprint') {
    pwsh -NoProfile -File tools/run_animation_visual_validation.ps1 `
        -Case $case -CaptureCount 0 `
        -OutputDirectory "docs/audit/bf150/scene-regressions/recheck-$case"
}
```

`runner.json` 保存完整判定与渲染回执。`report.sha256` 对应解压后的原始报告。原生脚部 socket 和不均匀时钟峰值可用 `python3 docs/audit/bf150/scene-regressions/replay_rig_socket_clock.py` 复算。 [时钟复算脚本](replay_rig_socket_clock.py)
