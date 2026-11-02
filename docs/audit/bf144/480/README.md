# BF144.480：完整 IK 权重 wrapper 与三骨消费者

承接476精确pre/post与479完整旋转核。fresh source wrapper、三骨Slerp、InverseTransformPosition/TransformPosition及global setter的Equals/dirty合同；1332源调用分1324 full、6 partial、2 zero。fresh1330 native pre-weight core参数/AB旋转/BC位置精确，1324 full与旧post保持；独立wrapper恢复全部1332三骨完整T/Q/S，含六个0.25/0.5/0.75历史。零权重在长度和solver前返回，没有伪造计算结果。

公开target unit重现1272实际满权重完整三骨输出，另20个当前q6两模式双腿×0/.25/.5/.75/1控制、10个source同厘米数字共同输入，共1302完整模型与公开结果精确。控制不是自然partial历史。共同source显式使用已经原生资格的骨长，隔离D165和初始数据；原1272仍保原隐式长度。source的float `1-SMALL_NUMBER`舍入为1，两端partial分支在本字段类型下等价，不登记差异。

source六partial与current十二partial控制的144记录固定同一独立source core TQS。后者core另与479相同current数字的native结果逐值核，未将post混合结果当core输入。移除core固定会混入126个solver变化，隔离Gate1拒绝，两个完整端点仍通过。三个wrapper数学位：1 source Slerp标量dot/InvSin复用（D250），2 source Dot4/NEON归一化（D248），4 source逆/正位置变换vector算法（D247）。

source域分别只回退上述三因素时，Q/T double变化为5/6、12/8、0/12；current米数字域为15/12、31/21、0/24。所有相应F32投影保持，source位置先投米再判F32；因素数量不相加，不当作可见故障。每项均有最小两腿partial原生反例：pre-weight core2和public2仍精确、source TQS101拒。另物理删除位置重建，两native partial平移失配而旋转仍精确，public零权重控制单列，不冒称该反例验证了public partial。

source请求TQS逐值等于三次ordered global写入后的原生消费者结果，不需要新增writer适配层。此处没有统计各次Equals skip/dirty分支，也没有据此关闭后代传播。fixture初版的JSON整数/浮点表示选择器和结构比较曾误拒，原始失败保留；仅按已证权重元数据和FTransform浮点字段类型校正，native请求字节保持，不计移植差异。

canonical源码、完整输入按bytes恢复，同target六库重编，完整factors/focused-counts/restored报告byte一致；去掉全量硬编码计数后完整报告也相同，最终Gate仍严格要求1332/1330/1302/1272。源码/reader/转换器/引擎/原始资产未改。全部native、请求、失败/恢复、输入、完整矩阵及manifest纳入Git，二进制仅缓存；gzip解压rawSHA/长度逐项核对。

项目根复验：run_python.ps1 -Name facts / prepare / prepare_native / compare / identity / finalize；run_native.ps1 -Label <new> -Request core；run_replay.ps1 -Label <new> -Build。消融Case slerp、normalizer、vector，最后Case restore并重编完整Gate；position-rebuild与core-hold源码/输入单独保留。自动流程复用已自验固定wall/no-progress watchdog。

全后代local/global传播、dirty/Equals执行计数、完整Rig/Root仍开。截图0、性能验收暂停。
