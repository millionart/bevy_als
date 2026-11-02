# BF144.478：实际 IK 位置核与算序

依赖476精确pre/current/initial与477自动骨长。Fresh source AnimationCore位置核、FVector dot/Normalize/GetSafeNormal与现有native公开probe直接对照；新原生2596份：1324源厘米实际满权重输入、1272 current原始米数字。后者只换轴，骨长float数字保持，不能称为source角色的厘米历史；probe字段名保留既有root_cm等schema，实际数值域由inputs.json的domain明确区分。

两个域都没有stretch、goal/pole退化、完全伸直、反向上骨或负投影分支。每个实际输入在四数学组合中由断言确认资格；模型只保留该真实分支。无消费者的plane-normal归一化没有新增；无信号的geometry适配/重复identity赋值已删，完整报告byte保持。未访问分支及恒false状态随后也删除，除该恒定字段外2596×4记录逐值保持。未覆盖分支保留开放，不能以未遇到就算通过。

mask0保持current XZY累加/分量除法，mask1只改XYZ累加，mask2只改乘sqrt倒数，mask3两者同时。Mask0在两域都逐值等于公开RigGeometry.solve；实际1272 current结果还与476原VM post位置精确。mask3全部2596原生joint/end精确，源域mask0/1/2有798/695/325个joint double差，当前同输入域811/671/333；所有end及F32位置均保持。归既有D249消费者，不吞并D165骨长单位因素、D242初始数据或不同上游pose，也不宣称恢复最终source/current姿态。

分别物理去source倒数与source累加，使完整source算术合同101失败；原1272 public位置资格仍通过。最小化夹具再去倒数同样失败；canonical字节恢复，同target/六库重编后完整最小报告与minimal基线byte相同。最初编译夹具的u8数组索引错误已修正，原失败保留compile-initial.failure.txt，不计移植差异。

复验：项目根run_python.ps1 -Name facts / prepare / prepare_native / analyze / identity / finalize；run_native.ps1 -Label <new> -Request request（新label避免覆盖source record）；run_replay.ps1 -Label <new> -Build。facts/identity依ALS_UE_SOURCE_ROOT与gitignored配置。原生与所有重放压缩前后rawSHA/长度精确；读数、请求、源码、对照、恢复、身份与manifest全部纳入Git，二进制只作缓存。所有自动流程复用已自验watchdog，固定wall/no-progress上限见报告。

只关闭当前真实分支的位置核；旋转/partial/descendant与完整Rig/Root继续开放。截图0，性能验收暂停，生产、reader、引擎、转换器及原资产均未改。
