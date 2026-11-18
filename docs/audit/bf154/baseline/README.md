# BF154.01 移植验收基线

验收范围见[移植差异报告](../../../parity-report.md)。[构建身份](identity.json)记录代码提交、锁文件和用于真实 Playground 的二进制。分层断言修复前后的源读数、红灯与加载角色对照见 [BF150.04a](../../bf150/overlay-test-recheck/README.md)。

默认并行全库复跑时，分层断言已通过。`zero_joint_ragdoll_completion_excludes_self_from_camera_queries` 出现一次失败，测试进程仅输出用例名。单独精确复跑 1 项通过，`als_camera::tests::` 并行复跑 26 项通过。验收沿用 BF146.06 的串行全量方式。相机生产行为依照已合入的源合同。

[串行全量输出](baseline-workspace-serial.stdout.gz)记录 2,127 项单元测试和 37 项集成测试通过。保留[并行观察](baseline-workspace.stdout.gz)与[相机模块复跑](baseline-camera-module.stdout.gz)以区分验收范围。

Playground 采用原 25／75／4 ms 帧预算与每帧最多 3 个物理步。首次启动和首次 Overlay 均通过功能复核，墙钟峰值分别为 2028.75 和 365.72 ms。随后同一二进制复跑，两场完整通过，峰值分别为 27.23 和 28.07 ms，图求值 p95 为 2.23 和 3.58 ms。两次 Overlay 均通过 17 项场景检查和 6 个动作窗口。各场图错误、姿态故障均为 0。原始报告可从 gzip 解压为 `report.json`，[结果表](runtime.json)给出压缩前哈希与字节长度。

```powershell
cargo test --workspace --locked --offline --jobs 2 --no-fail-fast -- --test-threads=1
cargo build --locked --offline --jobs 2 --profile play --example manual_playground
pwsh -NoProfile -File tools/run_animation_visual_validation.ps1 -Case startup -CaptureCount 0
pwsh -NoProfile -File tools/run_animation_visual_validation.ps1 -Case default_overlay -CaptureCount 0
```
