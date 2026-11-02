# BF144.479：实际 IK 主轴与次轴旋转

承接476的精确pre/current/initial和478已资格位置核。fresh沿SolveBasicTwoBoneIK读取TransformVectorNoScale、FindBetweenNormals、四元数乘法与Normalize的NEON实现；source厘米1324/current原米数字1272各有默认次轴1和Primary-only 0，共5192份native参数回显与AB旋转。全部5192位置保持；source默认1324完整原生结果与476实际post及478旧body逐值同。控制0不是实际默认历史。

完整独立源旋转模型5192×2 quaternion全native精确；当前模型在两输入域/两个次轴权重下均与公共RigGeometry.solve精确，1272实际默认AB旋转及BC位置另与原VM精确。默认模型的primary阶段还由独立Primary-only native结果校准，共2596组两骨，未把最终结果回填为中间输入。

mask1为source XYZ dot，2为向量倒数乘法，4为source quaternion-vector展开/FMA，8为source Dot4/NEON normalizer，16为source坐标基quat product。31是完整source；0是完整current且保current位置。其余30/29/27/23/15分别仅回退一个旋转因素，位置固定为独立source位置。去位置固定仍使两个完整端点通过，但混入4048条位置变化，隔离Gate1拒绝，因此保留该最小输入边界。

四组（source/current×默认/Primary-only）完整矩阵保留comparison.json。source默认共同输入的current核有4个F32 quat不同，仅回退vector rotate也有4个；当前默认共同输入有1个F32不同，回退vector rotate或normalizer各有1个。current Primary-only完整current核的F32都同，但向量分量除法单因素另有1个F32变化，不能称为默认故障或单因素修复。所有F32见证及实际mode/query/pc在witnesses.json；各因素作用不相加，source/current不同上游数据和D165长度单位阶段保持分离。既有D163/D247/D248/D249扩消费者，无新编号。

五个算子分别选取有真实原生差异的最小默认/Primary-only输入对。物理去算子后public两条仍精确，source AB原生旋转合同101拒；输入、源码、选择身份、失败与完整输出逐份保留。去掉全量硬编码计数、改为非空输入数后，完整5192报告byte同，最终Gate仍严格要求5192/1272资格。canonical源码和5192输入字节恢复、同target六库重编，完整报告与因素基线byte一致。

本实际域只经过普通非退化、非完全伸直、次轴权重0/1分支；任意部分次轴权重和退化没有据此算通过。位置复用478独立核，NEON复用471已资格实现，不新增生产适配层或运行时状态。这里只关闭实际solver AB旋转，IK权重partial wrapper与后代传播、完整Rig/Root仍开。

项目根复验：run_python.ps1 -Name facts / prepare / prepare_native / compare / identity / finalize；run_native.ps1 -Label <new> -Request orientation；run_replay.ps1 -Label <new> -Build。消融用run_python.ps1 -Name ablate -Case dot、vector-normal、rotate、quat-normal、product，随后run_replay.ps1；最后Case restore并同target重编完整Gate。压缩后输入和报告仍可复验，gzip rawSHA/长度核对；全部长期读数/请求/源码/失败恢复/身份/manifest在Git，二进制仅缓存。自动流程复用已自验固定wall/no-progress watchdog。

截图0、性能验收暂停；生产、reader、引擎、转换器及原资产未改。
