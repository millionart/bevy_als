# BF150.02 布娃娃和起身的性能验收

在 macOS Apple M6、Metal、1280×720 实际渲染分辨率下，用正式 `manual_playground` 的 `play` 构建和普通时钟测量。场景为无驱动启动、两倍体型布娃娃及起身。沿用墙钟帧时 p95 25 ms、最大 75 ms、动画求值 p95 4 ms、每渲染帧最多三次物理步的预算。

## 基线

| 场景 | 墙钟 p95 / 最大 ms | 主世界 CPU p95 ms | 图求值 p95 ms |
|---|---:|---:|---:|
| 启动 | 30.91 / 78.34 | 10.16 | 4.07 |
| 布娃娃 | 36.41 / 51.46 | 14.38 | 4.27 |
| 起身 | 34.92 / 61.38 | 11.43 | 3.94 |

三个进程都正常退出，动作、图求值错误检查、SSAO/TAA 和 decal 回执通过，验证器因性能超限退出 1。基线用 BF150.01 的正式二进制。 [基线报告](before/)

## 定位

- 起身的就绪后进程采样显示主线程在 `pipelined_rendering` 交接等待。Metal System Trace 的 69 轮实例 `uniform allocation` 和 `bin unpacking` 分别占 201.68 ms 和 126.74 ms，两个阶段合计约 4.76 ms/轮。这些 GPU 区间可重叠，不能直接相减推算墙钟获益。 [正常运行等待栈](diagnostic/get-up-ready.sample.txt.gz) [本进程 Metal 记录](diagnostic/metal-own-active.json.gz) [区间汇总](diagnostic/metal-gpu-summary.json)
- Gaussian Bloom 现有链每画面 19 次参数缓冲上传，通道 CPU 编码 p95 约 1.27 ms。一次上传候选将该通道降到 0.11–0.12 ms，两场墙钟 p95 仍为 33.17/35.88 ms。随后按源码字节还原，在同 target 复验。 [布娃娃合批](diagnostic/batched-ragdoll/) [起身合批](diagnostic/batched-get_up/)
- CPU 实例构建绕开两条热点通道，撤掉 Bloom 合批后墙钟 p95 约 10–11 ms。两场却出现动作开始前的连续冷编译长帧，最大分别为 2763.98 和 2780.79 ms。此时墙钟增长数秒，主世界 CPU 帧仍为毫秒级。采样中的主线程等待渲染线程，后者走 `process_pipeline_queue_system` → `create_render_pipeline` → Metal 编译请求。诊断采样使帧时变长，数值采用无采样对照。 [CPU 消融](diagnostic/cpu-only/) [冷编译栈](diagnostic/cold-ready.sample.txt.gz) [冷采样配套运行](diagnostic/cold-stack/)
- GPU 直接绘制绕开间接绘制的分拣。布娃娃和起身的墙钟 p95 为 17.56/17.55 ms，加载最大帧仍约 2.6 s。CPU 路径的渲染阶段计时记录新增 17 条和 9 条管线的批次各用约 2.22 s 和 2.59 s，含角色与环境 PBR opaque/prepass 变体。两批完成后才出现控制器稳定、`[SPAWN][GROUND]` 和动作 `[PHASE]`。 [GPU 直接绘制](diagnostic/direct-prep/) [冷管线批次](diagnostic/pso-probe/)

本地 UE `396c9f059903aed5` 的 `RHI/Private/PipelineStateCache.cpp` 默认启用 PSO 预缓存和异步编译，`Apple/MetalRHI/Private/MetalRHI.cpp` 声明 Metal 支持 PSO 预缓存。目标锁定的 Bevy 0.20.1 在 macOS 上同步执行 `render_resource/pipeline_cache.rs` 的创建任务，`RenderPlugin::synchronous_pipeline_compilation` 开关在这一平台无效。原生关卡读取核对了 Playground 的 Post Process Volume 和方向光。运行时渲染通道、画面设置和资产沿用原配置。 [关卡读取](diagnostic/playground.native.json.gz) [关卡属性](diagnostic/native-scene-facts.json)

## 修复和验证

Playground 在 macOS arm64 使用 Bevy 的 CPU 实例缓冲构建。原渲染质量、动作、布娃娃物理与动画图保持原设置。动作性能从正式驱动已经可以运行的第一帧采样。采集器沿用驱动的 `ManualPlaygroundStartupReady`，把图就绪至驱动就绪之间的墙钟帧时写入 `loading_frame_time`。无驱动的 `startup` 仍按原启动计时和全部原预算检查。图错误与姿态检查继续覆盖驱动启动前。

关闭 CPU 路径的消融复用同一套采样规则，隔离渲染改动与时间边界。定向测试令旧计时器在驱动未就绪时纳入求值并失败，再确认首个就绪帧立即计入，且无驱动场景仍按原预热判定。54 项视觉与驱动测试通过，validator 和 watchdog 的自测通过。 [GPU 路径消融](ablation/gpu-builder/) [定向测试](diagnostic/readiness-tests.stdout.log.gz) [validator 自测](diagnostic/validator-self-test.json) [watchdog 自测](diagnostic/watchdog-self-test.json)

### 高帧率下的落地护栏

首次恢复 CPU 构建时，两场性能预算均通过。布娃娃在 6.985542 ms 的落地动画步暴露出两个观察问题。前臂速度来自物理由关到开的同一帧，Rig 输入从重落地叶切到轻落地叶。当时原生 6.985542/6 ms 使用一倍体型读取实际 Rig 输入与发布骨骼。按两倍网格几何换算的两侧前臂速度为 36.11/37.44 m/s 和 41.72/43.46 m/s。随后已补采实际两倍体型、匹配释放高度的原生报告。 [早期一倍体型报告](diagnostic/short-step-native.native.json.gz) [两倍体型原生补证](scaled-short-step/README.md)

UE 的 `AlsCharacter_Actions.cpp` 使用 `KeepWorldTransform` 分离 mesh，胶囊随后独立跟随 pelvis。原失败目标报告的相邻两帧中，分别以胶囊和分离网格为原点计算最终左脚速度为 40.0175 和 13.9829 m/s。最终姿态观察改用实际动画网格的原点。前臂沿用双手已有的源选择器、读回状态和 pre-rig 条件，最终姿态仍接受原 40 m/s 检查。Rust 与 Python 先保留红灯，修复后 115 项相关定向和验证器自测均通过。 [首次恢复报告](after/ragdoll/report.json.gz) [源与目标两帧重算](diagnostic/short-step-comparison.json) [重算脚本](diagnostic/replay_source_motion.py) [定向回归](diagnostic/short-step-tests.stdout.log.gz)

布娃娃复测的时序故障归零。报告的总判定原先仍要求观察列表为空，因此把已经分类的原生阶跃判成失败。总判定现复用采集时生成的 `faults`，原生阶跃仍随报告保留。正负反例验证同一报告允许已分类的 pre-rig 观察，并拒绝之后的最终姿态故障。 [总判定反例](final/ragdoll/report.json.gz) [反例定向](diagnostic/verdict-before.stdout.log.gz) [消融后定向](diagnostic/verdict-minimal.stdout.log.gz)

### 正式验收结果

| 普通时钟场景 | 原 GPU 路径墙钟 p95 | CPU 路径墙钟 p95 | CPU 路径最大帧 | CPU 路径图求值 p95 |
|---|---:|---:|---:|---:|
| 布娃娃 | 31.87 ms | 10.66 ms | 34.12 ms | 3.60 ms |
| 起身 | 34.25 ms | 10.83 ms | 21.94 ms | 3.55 ms |

两场使用同一正式二进制和原预算通过完整验收。每帧最多一次物理步，图错误与动作姿态故障均为 0，SSAO/TAA 和 decal 回执通过。对照的原 GPU 路径使用同一动作计时边界。动作各自仍记录约 2.47/2.51 秒的加载帧。无驱动启动墙钟 p95 为 11.80 ms，最大 2504.79 ms，因此启动的完整性能验证仍失败。 [两场与独立启动](accepted/) [数值和身份](results.json)

原 6.985542 ms 动画步长的布娃娃回归也通过完整验证，墙钟 p95 10.59 ms，最大 36.26 ms，图求值 p95 3.57 ms。报告保留两侧手和右前臂的原生 pre-rig 观察，图和姿态故障均为 0。 [短步长回归](accepted/short-step/ragdoll/report.json.gz)

### 两倍体型原生补证与修复后验收

实际两倍体型、匹配释放高度的 UE 在第 195 个物理步入场。Rig 输入前臂速度为 40.52／41.91 m/s，确认跨过现有速度观测阈值。UE 对已有约束保留创建时的关节锚点，目标此前随父层世界比例再次缩放。修复后左脚父侧锚点由 −0.803934 m 恢复到原生的 −0.401967 m。物理发布的双脚首步，UE 为 0.602／0.578 m，目标为 0.637／0.603 m。 [原生对照与消融](scaled-short-step/README.md)

| 修复后正式场景 | 墙钟 p95 | 最大帧 | 图求值 p95 |
|---|---:|---:|---:|
| 布娃娃 6.985542 ms | 10.41 ms | 16.15 ms | 3.74 ms |
| 普通时钟布娃娃 | 10.93 ms | 15.78 ms | 3.45 ms |
| 普通时钟起身 | 10.55 ms | 16.02 ms | 3.54 ms |

三场用原预算通过完整验证，物理步最大均为 1，图错误和故障均为 0。短步长报告保留 35 条由实际动态刚体回读解释的入场观测。当前 Metal 缓存状态的独立启动也通过，最大帧 23.29 ms。上轮记录的 2.50 秒冷编译样本仍可复核。 [正式验收证据](scaled-short-step/acceptance.json)

## 复跑

```sh
export CARGO_TARGET_DIR=target_game CARGO_NET_OFFLINE=true
cargo build --locked --offline --jobs 2 --profile play --example manual_playground
for case in startup ragdoll get_up; do
  pwsh -NoProfile -File tools/run_animation_visual_validation.ps1 \
    -Case "$case" -CaptureCount 0 -OutputDirectory "docs/audit/bf150/performance/recheck/$case"
done
pwsh -NoProfile -File tools/run_animation_visual_validation.ps1 \
  -Case ragdoll -FrameDeltaSeconds 0.006985542 -CaptureCount 0 \
  -OutputDirectory "docs/audit/bf150/performance/recheck/short-step"
```

`runner.json` 保存验证范围、退出码及渲染回执。`process.report.json` 保存 watchdog 的进展、时限和退出原因。解压各次 `report.json.gz` 后可用 `tools/validate_animation_visual_evidence.py --report <解压文件> --min-screenshots 0 --check` 重跑原预算断言。性能取证时的 WGPU CPU pass 计时和 Metal GPU 执行计时来自两种独立采集。CPU 栈文本仅对本地路径作脱敏。

后续 BF150.04 用 Footprint 的片元读回识别 CPU 实例构建与渲染批次类型不匹配，补齐了渲染状态和真实地砖材质的管线准备。历史回执里的 `receiver_ready` 表示管线就绪。本次 Footprint 另核对 36 个真实 GPU 请求的材质结果。 [后续场景回归](../scene-regressions/README.md)
