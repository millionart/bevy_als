# BF144.532 原宽支撑有效、窄胶囊失败与查询阶段

16 cm 墙、Falling 初态和低台高度保持。低台 X 中心相对场景原点为 2.30、2.48、2.64 m。三案实际原生首墙命中一致，宽胶囊都查到可走墙顶。近距窄胶囊可达低台，源保墙顶，第 49 帧实际 Landed。中距与远距的窄胶囊无命中，源保宽命中数据并撤销 walkable，第 49、50 帧仍 Falling，第 51 帧收到墙顶 Landed。关闭原生观察的 52 帧运动、状态和姿态一致，状态引用只规整临时世界序号。

目标在原生停止点、目标停止点及状态同步前的公开查询与重放逐值。中距在两个停止点均撤销支撑，滑动后窄胶囊才够到低台，保留宽墙顶并于第 49 帧发布 Grounded。停止点支撑控制只在这种晚查成功时抑制墙 contact，保持实际 Falling 事务、位置、速度和帧末候选，中距因此也在第 51 帧发布 Grounded，近距仍为第 49 帧。远距三个位置均无窄命中，实际第 51 帧落地；跳过窄胶囊的诊断会留下间隙小于 2.4 cm 的可走墙顶。差异归 D292 的查询阶段。

恢复后关键三帧一致，完整参数最早在第 54 帧分开。共用 Rig、骨架、配置和自测沿用 .522 的相同证据。截图为 0，性能验收暂停。

封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/532/run_python.ps1 -Name verify`，用 `ALS_UE_SOURCE_ROOT` 提供本地 UE 源码。原生复现依次调用 `prepare_request`、`prepare_reader`、`build.ps1 -Label offset` 和 `native.ps1 -Label wall -Request wall`。Playground 复现用 `observer` 和 `compile.ps1 -Label offsets`，以 `-HalfWidthCm 100 -Frame 49 -WallCm 162` 运行 `near`、`mid`、`far`、`mid-stop-guard`、`near-stop-guard`、`mid-restored` 及各自功能验证。重采用空输出目录。
