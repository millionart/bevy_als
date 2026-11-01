# BF144.456：真实 ControlRig VM 与独立输出入口

沿455已证Head→ControlRig→Ragdolling→Root，在原可见Playground、同一target_game六库读取实际状态。319组after、318组已有before状态：每组405个寄存器、12个external、11个真实输入绑定、79骨层级；全部25,201个层级local转F32后精确匹配独立Rig输出。Head输出到Rig输入全Pose319组精确，318组before状态精确接续前after，初始化次数始终1；共13,378次unit调用、636次实际trace。

调用参数是unit执行后的参数（含输出），`bones_before_call`来自调用前层级副本；不把二者混成输入。upstream context曲线按其实际身份记录，不冒称未公开的private曲线map。所有寄存器、external引用及当前property/global优先级逐值核对，六个错误数据fixture拒绝，包括非有限数被JSON编码为null的情况。额外未映射SKM、weights与Pose metadata在Rig边界保持。

与455相同的318正步长域，观察到的变化骨行如下；有重叠，不能相加。

| 边界 | T | Q | S |
|---|---:|---:|---:|
| Head/RigSource→Rig输出 | 1572 | 2011 | 819 |
| Rig输出→Ragdolling输出 | 0 | 3714 | 0 |
| Ragdolling输出→Root | 0 | 0 | 0 |
| Head/RigSource→Root整体 | 1572 | 5184 | 819 |

额外初始化query的Ragdolling另有14个旋转变化，故全319组该边为3728。这里只分出当前写入边界，尚未证明具体source算法原因；尤其不能把全部Root旋转变化归给Rig。source独立post-Rig与LocomotionAction仍待取证。

设计消融纠正了两个多余设计。首次尝试改普通global capture=false时，实际calls仍22/42，原因是`publish_control_rig_inputs`发布了更高优先级的source-frame输入；已有视觉遥测本来已开启capture。因此删除新增global setter并恢复immutable player查询。`elapsed_seconds`经fresh源码确认是CPU Instant耗时，依性能暂停从新增观察删除，原prototype原始报告仍保留。删除二者后，旧五数组及所有实际VM/Pose/输入/call/trace精确保持。

正确关闭已有`AlsFootIkTelemetryEnabled`后，319组actual calls确实全部空，合同先因实际数据缺失失败；除call/trace与来源字段，所有实际VM/Pose/输入仍精确。删除新增观察字段也明确失败。三源码按canonical保存字节恢复，同target重编；恢复后完整新旧语义数组、检查、分区、definition与skeleton字节精确，两次原场景functional-only通过。原性能报告和阈值保留，预算问题仍deferred，不能称full/performance通过。

复验：`compile.ps1 -Label <new>`，`run.ps1 -Label <new>`，`run_python.ps1 -Name check -Case <new>`与`partition`；公共`functional.ps1`保持截图0/functional-only。`ablate.py`支持`no-capture`（关闭现有真实遥测resource）、`no-observation`、`restore`。所有长期源、参数、报告、最小失败、消融与身份均在Git目录；二进制与普通日志可重建，日志不是唯一证明。源码、reader、引擎、原资产与图保持不变；无新D，272项未修，完整Rig/Root/ALS审计继续。
