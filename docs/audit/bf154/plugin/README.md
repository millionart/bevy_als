# BF154.05 插件交付回归

默认库、带 `playground` 的 Playground、以及最小角色例程使用同一个 Cargo 包。[编译输入与产物身份](isolation.json)记录默认消费者和 Playground 的依赖文件。前者仍编译角色与脚印接收器，只有后者编译演示场景、HUD 和三项验收工具。

[独立宿主夹具](../consumer/README.md)从自己的 `game-assets` 目录加载 ALS、ALSRuntime 和 ALSCamera，预装同一物理组与 Enhanced Input。重新链接默认库后，标签、角色和相机就绪，角色通过状态请求进入蹲姿，再由键盘输入产生 0.259 m 移动。

带 `required-features` 的实际 Cargo 夹具用于验证启动器。旧参数在默认目录、环境变量、Cargo 配置三种场景均缺少 feature 而失败。加入参数后，包含三种目录模式的 15 项平台测试通过。公共入口沿用相同的 `run_manual.ps1` 和报告验证。

```powershell
cargo test --workspace --locked --offline --jobs 2 --features playground --no-fail-fast -- --test-threads=1
cargo build --locked --offline --jobs 2 --profile play --features playground --example manual_playground --example als_character
python3 -B tools/tests/test_platform_tools.py -v
```

[全量串行输出](final-workspace.stdout.gz)记录 2,128 项单元测试与 37 项集成测试通过。[贴花验证器自测](decal-validator-self-test.txt)通过 9 个反例。

同一正式二进制通过以下五场的原 25／75／4 ms 预算和每渲染帧最多三步物理检查。功能与姿态故障、图错误均为零。布娃娃保留 34 条已归类的源交接观察。

| 场景 | 墙钟 p95 (ms) | 最大墙钟 (ms) | 图 p95 (ms) | 动作窗 |
| --- | ---: | ---: | ---: | ---: |
| `startup` | 17.80 | 32.33 | 2.32 | 0 |
| `default_overlay` | 18.59 | 30.72 | 3.26 | 6 |
| `footprint-warm` | 18.73 | 52.28 | 3.00 | 10 |
| `ragdoll` | 18.50 | 22.94 | 2.45 | 1 |
| `get_up` | 18.59 | 23.90 | 2.22 | 2 |

首次 Footprint 通过功能验收，墙钟峰值达 491.23 ms。暖态复跑恢复完整预算。两次 GPU 验证均覆盖 36 次请求和 576 项采样。各 `report.json.gz` 解压回 `report.json` 后可用原验证工具复算。 [运行结果、压缩前哈希和长度](runtime.json)
