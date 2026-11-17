# BF151.01 — 蹲姿动态过渡夹具

`dynamic_left` 超时来自验收输入。旧夹具先打一帧侧向移动，再通过 Quick Stop 回到 Idle。Quick Stop 可以成功，但不会保证后续允许动态过渡时左脚仍有足够的锁定位移。四个独立的 Stop／Quick Stop 窗口继续验收。

## 本地对标

[原生 Crouching 蓝图](crouching.native.json.gz)在 `Crouching States / Idle` 的 On Update 调用 `RefreshDynamicTransitions`（GUID `d6d25a2d-48c1-9cb8-e907-22934e2b5874`）。`AlsLinkedAnimationInstance.cpp:162` 转发到父实例。父实例在 `AlsAnimationInstance.cpp:1675–1799` 先检查 `AllowTransitions`、更新和帧冷却，再比较脚的锁定量及锁位置到当帧目标位置的距离。双侧都合格时，距离更大的一侧胜出，相同则选左。它把所选序列排队给游戏线程的 `Transition` Slot。

[原生默认设置](settings.native.json.gz)给出 8 cm（乘角色世界缩放）、1.5 倍播放、两端各 0.2 s 混合和 `A_Als_Crouch_DynamicTransition_{Left,Right}`。目标的 `source_native_refresh.rs`、`state/transitions.rs` 和 `transition_notify.rs` 分别对应 Idle 调用、选边、排队和 Slot 播放。此次只改 Playground 的输入夹具。

[原生读取请求](source-request.json)固定两个资产及读取开关；[文件校验](compressed.json)记录压缩前字节数和 SHA-256。

## 原始反例

| 同一旧夹具 | 允许过渡时左脚位移 | 左侧结果 |
|---|---:|---|
| [BF148.02](../../bf148/02/crouching_actions/runner.json) | 最大 6.33 cm | 未排队，超时 |
| [BF144-U07](../../bf144/u07/standing-ground-clearance/crouching-actions/runner.json) | 最大 6.78 cm | 未排队，超时 |
| [`b070df4c` 普通时钟](baseline-runtime/runner.json) | 排队时 8.01 cm | 完整通过 |
| [`b070df4c` 固定 30 Hz](baseline-30hz/runner.json) | 最大 6.50 cm | 仅左侧超时，退出 91 |

两次本轮基线由正式脚本启动同一个已构建的 `target_game/play/examples/manual_playground`，二进制 SHA-256 为 `285bd6ed812a5a96696a7631d8519b8a21585ceca10c2e667354b996ed586d26`（与 BF150.04d 恢复构建一致）。基线期间同一工作区正在更新依赖锁；预构建入口由临时 Cargo 启动转发执行，按正常 Cargo 环境带上项目的 `CARGO_MANIFEST_DIR`。使用 30 Hz 只固定游戏时钟；墙钟、图求值和物理步预算由公共验证器原样检查。

位移来自原始 `manual_integration.direction_trace` 的 `dynamic_inputs`（米平方）；筛选相应窗口、`transitions_allowed` 和锁定权重 > 0.00001 的帧，再开方换算为厘米。`native_dynamic` 中当帧索引等于最后写入索引时表示本帧排队。两份旧失败和本轮 30 Hz 失败在左侧窗口都没有该事件，图报错数为 0。

## 修复和验收

角色 Idle 且两脚完全锁定后，夹具绕对侧脚的目标位置旋转角色 30°。以角色的实际位姿变化提供两侧位移，运行时继续按较大的合格距离选边，并通过原有 Transition Slot 播放。没有改写脚锁、序列或播放观察。定向消融对同一锁脚姿态去掉这次位姿扰动，两侧都不触发；应用扰动后分别选出两条蹲姿序列。[23 项定向测试](focused.txt)通过。

| 完整 Playground 场景 | 首次左侧排队，左／右距离 | 首次右侧排队，左／右距离 | 帧 p95／峰值／图 p95 |
|---|---:|---:|---:|
| [固定 30 Hz](fixed-30hz/runner.json) | 23.54／11.55 cm | 8.13／16.78 cm | 17.57／39.40／2.09 ms |
| [普通时钟](fixed-runtime/runner.json) | 24.69／9.96 cm | 6.99／16.78 cm | 17.52／36.32／2.01 ms |

两次均为 11/11 项覆盖、8 个有效动作窗口，最大物理步数为 1。两侧都观察到实际 Slot 姿态变化、解锁和重新锁定，图错误、姿态故障和时序异常均为 0。[机器可读结果](runtime.json)记录帧预算、排队输入、窗口与二进制身份。正式两次使用同一个 `target_game/play` 二进制，依赖锁包含独立的 [BF152.01](../../bf152/01/README.md)。

## 复跑

```powershell
pwsh -NoProfile -File tools/run_animation_visual_validation.ps1 -Case crouching_actions -CaptureCount 0 -FrameDeltaSeconds 0.0333333333333333 -OutputDirectory docs/audit/bf151/01/recheck-30hz
pwsh -NoProfile -File tools/run_animation_visual_validation.ps1 -Case crouching_actions -CaptureCount 0 -OutputDirectory docs/audit/bf151/01/recheck-runtime
$env:CARGO_TARGET_DIR = 'target_game'
cargo test --locked --jobs 2 --profile play --lib dynamic_
```

原报告无损保存为 `report.json.gz`。解压到临时文件后可用 `python3 tools/validate_animation_visual_evidence.py --report <file> --min-screenshots 0 --check` 复验。
