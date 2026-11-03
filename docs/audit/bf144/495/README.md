# BF144.495 左虚拟足局部微量缩放

真实 Playground 的首个非均匀 Head 输出在 query2。缓存从 Default Overlay 的 Idle 局部加性到 Head 的 Look 网格加性，显示两步各贡献一个 F32 步长。二者的普通 Stand 参考读取相同 glTF scale。原生 raw Stand 为单位缩放，匹配实际 Idle 和 Look 压缩加性读数的零 scale。公开局部和网格差分及施加算子逐值复算。只修其中一个参考还保留一个步长，两个都用源 raw 参考时该样本为单位缩放。归入既有 D235。

运动 query105/158 仍有更早的 Locomotion 输入缩放。本项记录实际缓存边界，保持这些输入来单独验证 Head 增量。

原生读取以 `native.ps1 -Label stand` 等执行。目标运行使用 `run_python.ps1 -Name observer`、`compile.ps1`、`playground.ps1` 与 `functional.ps1`。分析复跑需解压原始报告并设置 ALS_UE_SOURCE_ROOT。执行 `run_python.ps1 -Name prepare` 和 `run_replay.ps1 -Label baseline -Build`。将 raw_reference 替换为 recorded_reference 后，`no-raw-reference` 应触发单因素控制失败。恢复字节同库重编后报告相同。
