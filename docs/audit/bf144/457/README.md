# BF144.457：源端独立 Rig 输出与动作字段

在既有CharacterAnimationProbe增加严格bool opt-in `capture_rig_output`，要求同时启用`capture_pose_boundaries`。原请求省略新flag时保持原行为。新能力沿实际Root→Ragdolling→ControlRig链接，只读取得Rig输出和角色`LocomotionAction`的`GameplayTag.TagName`，不修改源资产、角色动作、生产或引擎。

原318帧主案例与2帧控制案例、各自初始化共322包/25,438骨行。六个Pose边界的骨架、帧、delta和计数均核对；删去新Rig/action字段及局部reader controls后，完整旧character probe逐值精确。主318域源Rig输出到Root的全部T/Q/S原始F64均不变；动作在采集时全部为None。动作记录是Pose边界序列化时点，带实际engine frame，不冒称Ragdolling Update已消费的值或实际Ragdoll分支覆盖。

动作getter由实际FNameProperty读取，使用同一GameplayTag类型的局部struct测试空值与非空Ragdolling tag；没有向角色注入动作。硬编码None会使非空控制失败。移除实际Rig输出观察链接会使初始边界缺失并退出7；移除parent guard会错误接受`capture_rig_output=true/capture_pose_boundaries=false`。严格类型/parent非法请求在最终包均退出7。

首次插入父InputPoses未采到数据：fresh `SetDynamicLinkNode`源码证明它复制链接，真实LinkedInputPose已持有旧副本。现包围实际linked实例中的同名InputPose，保存并按同owner恢复其原链接；主图/类型/LinkID/owner均验证，不按骨名或图ID写专用求值算法。初次EName/FName混合初始化列表编译错误、checker漏掉旧2帧控制案例，以及恢复构建未结束就过早触发读取的失败均保留；修正后重新核对实际package，失败序列没有原生报告被接纳。

源码按字节恢复，在同一native package重建；完整新增character probe、check和分区精确恢复。旧五边界完整请求、旧默认character请求和三轴CurveVector完整probe分别原生重读并逐值比较；Reader身份显式迁移，其余395文件和六个target库不变。标准watchdog记录进展并固定超时，沿用既有已测试实现。

复验：`build.ps1 -Label <new>`、`package.py <label>`、`run.ps1 -Label <new>`，再运行`run_python.ps1 -Name check -Case <new>`与`partition`。`ablate.py`提供no-output-hook、constant-action、no-parent-guard、restore；每次改动后必须等待构建终态，再核对package字节并读取。`compatibility_runs.ps1`串行执行全部兼容与非法请求，`compatibility.py`逐值核对；source事实/identity需要`ALS_UE_SOURCE_ROOT`。

原始报告、请求、源码前后身份、最小失败与无损压缩证明均在Git管理目录；build/log可重建，不作唯一证据。原动作空值不证明已覆盖其他动作，下一核源/目标Ragdolling实际图与共同输入normalization边界。无新D，272项未修；完整Rig/Root/ALS目标保持活动，截图0、性能暂停，GPU周边排除。
