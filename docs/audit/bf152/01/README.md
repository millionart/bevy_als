# BF152.01：Tnua 官方发布版

`e763953` 和 [上游修复 `1891a61`](https://github.com/idanarye/bevy-tnua/commit/1891a616ec91b935413b2800d8d387b7bd3920df) 都从 `83c4785` 分出。fork 唯一的修改是 Avian 3D 马达遍历遇到 `Disabled` 或 `SenseOnly` 时改用 `continue`。官方 `bevy-tnua-avian3d` 0.13.2 包含同一修改。旧版的最小失败和修复对照见 [Bevy 0.20 迁移证据](../../bf144/u07/bevy-0.20/README.md)。

[发布包记录](source.json) 包含官方归档地址、SHA-256、`.cargo_vcs_info.json` 和比较范围。可逐包下载并核对 `Cargo.lock` 的校验和，将 `src/` 与 fork 对应 Git 路径下的 `src/` 递归比较。43 个文件逐字节一致；发布包的 `Cargo.toml.orig` 对照 Git 中的 `Cargo.toml`，只有 Avian 3D 的版本从 0.13.1 变为 0.13.2。四个包统一从 crates.io 解析，外部依赖和其锁定版本均保持一致。

```powershell
cargo tree --locked --package bevy-tnua --package bevy-tnua-avian3d --depth 1
$env:CARGO_TARGET_DIR = 'target_game'
cargo build --locked --jobs 2 --profile play --example tnua_toggle_gate --example manual_playground
cargo run --locked --jobs 2 --profile play --example tnua_toggle_gate
pwsh tools/run_animation_visual_validation.ps1 -Case startup -FunctionalOnly -OutputDirectory docs/audit/bf152/01/startup-repeat
```

[锁定依赖](dependencies.txt) 保留 Bevy 0.20.1、Avian 0.8.0 和共享的 integration-layer 0.14.2。[马达回归](toggle.txt) 的三个启用角色同时移动，恢复后五个角色均移动。[Playground 启动](startup/runner.json) 含 90 个姿态样本，图错误、姿态故障和缺失骨骼均为零。启动器的 BF151 场景驱动在这次无驱动检查中未安装。

[验证记录](verification.json) 保存命令、watchdog 结论、二进制和原始报告的 SHA-256。用 `gzip -dk docs/audit/bf152/01/startup/report.json.gz` 恢复原始报告；启动日志中的本机工作区路径已替换为 `.`。
