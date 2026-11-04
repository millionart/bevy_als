# BF144.534 窄胶囊距离上限的数值边界

沿 .533 的 100 cm 主台与实际 Walking 初态，只有低台高度变化。源读数区分原始窄胶囊命中与距离检查后的资格，重查使用 Walking 范围。47.3 和 47.5 cm 两个对照都命中可走低台，前者首步仍有支撑，后者首步失去支撑，两端一致。原生关闭观察的两个对照各 43 帧运动、状态与姿态相同，状态对象引用只规整临时世界序号。

47.403 cm 见证中，源 AboveBase 加窄距离为 47.40260 cm，超过 47.40010 cm 上限，第 40 帧首步转 Falling。目标同二进制两次约为 47.39612 cm，保留支撑并在第二步贴低台，源与目标的帧末高度相差约 0.470 m。

将目标实际窄胶囊尺寸、低台盒尺寸、方向、扫掠长度和相对起点送入原生 Chaos，得到 42.50580597 cm；目标原始距离为 42.49892831 cm，差约 68.78 µm。目标命中双方见证点仍分离约 68.90 µm。只换原始距离的标量复算将目标总值改为 47.40300 cm，已超限。结果归 D125 的窄胶囊上限消费者。共用 Rig、骨架、配置和自测沿用 .522 的相同证据。截图为 0，性能验收暂停。

封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/534/run_python.ps1 -Name verify`，用 `ALS_UE_SOURCE_ROOT` 提供本地 UE 源码。重采原生基础对照用 `prepare_request`、`prepare_reader`、`build.ps1 -Label distance`、`native.ps1 -Label wall -Request wall`，边界用 `prepare_boundary`、`native.ps1 -Label boundary -Request boundary`。Playground 用 `observer`、`compile.ps1 -Label distance`，以 `-HalfWidthCm 100 -Frame 40` 运行 `depth-473`、`depth-475`、`depth-47403` 和 `depth-47403-repeat` 及各自功能验证。取得目标边界报告后运行 `prepare_geometry` 和 `geometry.ps1`。重采用空输出目录。
