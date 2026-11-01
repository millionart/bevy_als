# BF144.445 当前完整叶播放器时钟表

`declarations.json` 将85个 native player GUID逐一映射到唯一目标声明。读取每个当帧完成的 Default/Temporary key，记录 Sequence/explicit evaluator、BlendSpace 与内置 ClipNode 子节点的时钟；未实例化与未更新的声明不补零。source 的实际 tick 资格及 normalized accumulator来自已提交 `.445a`，current_asset_time是最高权重样本的秒数。

原 Playground 当前240有2805个完成叶 key，其中1315个为内置Clip、14个原GUID与18个helper GUID实际更新。当前Temporary0，公开反射focused fixture验证temporary-only、default+temporary、仅started、缺cache、next-frame清理；不能把fixture说成真实场景已触发临时状态。

source1560/target1490实际原GUID tick、49个目标声明未实例化、621份各自历史下的paired秒差保留在完整player表。两侧初始化/物理前态及ready前历史不同，不能由这些配对直接确认新缺陷或声称时钟等价。目标最早Absolute0、ready前79更新与source正world步的资格边界保持。

fresh Idle原资产的同长/rate1/无marker、source唯一Secondary Motion成员已核。公开tick独立重放当前319步；直接float秒累加匹配source240。对齐正步数后，第6步共同previous=0.1666666716337204、delta=0.03333333507180214：source0.20000000298023224、target0.19999998807907104。235次差是既有D201的新当前cohort，未新增编号；其余paired差不全部归D201。

首轮label `baseline`没有内置Clip子节点，故只作有限旧字段保持控制，完整验证器必须拒绝。`baseline-full`才用于完整Gate。消融删除leaf采样、过滤temporary key、移除直接秒因子，分别被结构Gate、focused fixture与native240重放拒绝。恢复源码字节、同target六库重编并复跑原场景；完整数组与模型输出精确恢复。截图0、性能暂停；GPU pass按用户周边排除未验收，公共动画functional-only通过。

复验：`pwsh docs/audit/bf144/445/run_python.ps1 -Name check -Case restored`，`pwsh docs/audit/bf144/445/run_fixture.ps1 -Label restored -Build`，`pwsh docs/audit/bf144/445/run_replay.ps1 -Label restored -Build`。所有长期请求、原生原始报告、完整表、消融/恢复结果、source SHA与manifest在Git管理的本目录或明确引用的`.445a`，构建缓存不是唯一证据。完整同历史Root、非零view和custom attributes继续开。

Idle原资产读取请求为 `pwsh tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Base/A_Als_Idle.A_Als_Idle -Output docs/audit/bf144/445/fresh-idle.native.json -TimeoutSeconds 180 -NoProgressSeconds 90`；本次原始结果为 `idle.native.json`，配套 reader watchdog报告同目录保留。复验另用新文件名，不覆盖原始结果。
