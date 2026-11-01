# BF144.442c2：cold fallback 角度来源

`.442c0/restored.native.json.gz` 的实际 local controller 首帧前快照给出：manager 存在、cache time 0、viewtarget 是角色、control yaw 60°、实际 getter 与 viewtarget actor yaw 都为 0°。本项不调用 Enhanced Input 事件；按 fresh `Input_OnMove` 的直接组合，以相同轴 `[0,1]` 将这两个实际角度交给现有原生数学探针，固定输入与数学，仅改变角度字段。

| 条件 | source 实际合同 | target 所读字段 / 本项证明 |
| --- | --- | --- |
| Controller、manager 存在，cache time 0，target 为角色 | `GetPlayerViewPoint` 取 viewtarget actor rotation | 无 published view 时取 raw control；本项 native 0°/60°反例 |
| manager 缺失 | `GetViewTarget` 回 controller 自身；root absolute rotation，`SetControlRotation` 更新 root | 默认 controller 语义跟随 control，不能一律按 pawn 角判差；仅 source/static 合同 |
| cache time > 0 | 取 camera cache POV | published view 正常消费由 .442a/.442c1 验证；本项没有 source native warm 生命周期 |
| Controller 无效 | `Input_OnMove` 保留 Character ViewState rotation | source 普通 UnPossessed 销毁玩家 InputComponent；本项未动态执行无 controller 的手动非零输入，不宣称普遍生命周期等价 |

source 两次原生方向分别为 runtime `[1,0,0]` 和约 `[0.49999994,0,0.86602545]`。target 使用同物理角映射 `phi=-theta-pi/2` 和同一个公开方向核，current 为约 `[0.50000006,0,0.86602539]`，只替换 selected angle 后为 `[1,0,4.37114e-8]`。固定公开 grounded 消费条件，X 计划由 `0.33333337` 变回 `0.66666669`，后者等于 native actor input 的同核 X 计划。侧向残差仍为既有角度表示/数学差异，未声称全部向量或世界反馈恢复。

这不是 D269/D270/D271：两端各自冻结数学、输入幅值和 consumer，只改所选字段仍改变大角度方向。source actor 0°与 control 60°已经在真实 getter 快照中区分。`.442c1` 的真实相机失效第20/21帧证明 actual drive 无 published 时消费 raw yaw；它们的 actor yaw 是另一个值，不能将 camera-off 当成 source cache0 同条件，也没有观察到自然 cold 非零输入。上述限制不阻止登记可重复的 selector 合同差异 BF144-D272/P2，但禁止称为自然游戏位移故障。

`matrix.rs` 只复算公开方向和公开 grounded 核，actual getter 取自封存快照，actual consumer 取自原 Playground。消融将 focused 核的 selected angle 从 raw 改为 source actor，原反例断言须退出101；源码字节恢复同 target 库重编，完整结果应与 baseline 字节相同。四个 cold 条件负fixture拒绝错误的 owner/manager/cache/返回字段。

```sh
pwsh -NoProfile -File docs/audit/bf144/442c2/run_matrix.ps1 -Label replay-build -Build
pwsh -NoProfile -File docs/audit/bf144/442c2/run_matrix.ps1 -Label replay
pwsh -NoProfile -File docs/audit/bf144/442c2/run_python.ps1 -Name check
```

原生请求/报告与 watchdog、source 绑定/控制器/解除绑定事实、focused 程序/结果、消融/恢复和身份清单均 Git 跟踪。本项不修生产、图或资产。截图0，性能验收暂停。
