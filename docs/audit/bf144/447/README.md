# BF144.447a 控帧完整余差库存

本批使用已封存source `.446f`新engine帧318与target `.446g` actual319，重新验证318输入/实际controller、source native刷新和target App帧资格。source旧同engine帧、target错位以及4个输入轴/时间/controller负例被拒；旧同帧的计数不沿用。

85声明新表有source1980/target1879 tick、101资格差/697秒差。启动Default_Poses与Layering首资格已匹配，最早资格差后移到source117的Acceleration。排除已有D201的Idle，最早clock差为source78/target query79（ready0）的Look：0.5098476409912109/0.5098477005958557。表格不将不同内部前态下的配对直接定为新D。

fresh原Head图/RON证明Look phase消费head_state.yaw_amount，X消费pitch_angle；原Settings读取Head yaw half-life为0.10000000149011612，与当前默认相符。源SpringMath与target当前实现已按实际代码记录，InvExpApprox同为展开形式，未凭猜测指成Horner差异。当前target真实Head回调前后状态与所有消费参数未记录，不能仅从sample_time推断kernel根因；后.b只补这条直接闭包观测，必要完整Root pose另按读数拆分。

source内部float/double使用numeric_value，外部请求/报告/身份引用均有Git路径。完整Root/非零view与custom attributes继续开，271确认差异不变，截图0/性能暂停，周边GPU不验收。

复验：`pwsh docs/audit/bf144/447/run_python.ps1 -Name phase_check`、`-Name compare`、`-Name clocks`。Source资产读取原请求为 `pwsh tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/AB_Als_Head.AB_Als_Head -Output docs/audit/bf144/447/fresh-head.native.json -TimeoutSeconds 180 -NoProgressSeconds 90`，Settings将Asset换成`/ALS/ALS/Data/AnimationInstance/AIS_Als_Default.AIS_Als_Default`并用新文件名。保留本次raw报告，不覆盖封存过程元报告。
