# BF144.481a：原生原始层级缓存边界

481传播审计前置，承接480三骨writer请求与最终global已精确。现有boundary只给typed三骨求值后的global，无法证明其它后代的cache/dirty生命周期。此项在原native函数后、任何既有item getter前，新增hierarchy_cache_before_item_reads；按全部key只选FRigTransformElement，读取name/type/parent、current local/global dirty、原始local/global storage。当前CR_Als为79 Bone/43 Curve，只记录79有效transform。

fresh UE const GetTransform返回PoseStorage，FRigComputedTransform::Get直接返回Storage引用，GetDirtyState仅bool引用，不求值、不清dirty。没有新增pose getter、配置或持久运行状态。GetPoseVersion虽是const，但会扩张计数数组并启用后续tracking，明确不调用。dirty缓存值是原始存储，不冒称当前已求值姿态。

四个完整旧请求保持字节：actual两个319历史与5个7帧权重控制，分别Pole前/IK后边界。4010快照、316790骨cache记录全部source schema/拓扑/flags通过；删除新增数组后，原执行、pose、work、hierarchy、typed items和descriptor恢复精确，仅复用462已证明的transient cache identity处理。10011个clean typed global缓存与原getter结果精确；1330有效腿IK后未直接写入的ball global dirty/local clean，作为该资产/fixture事实，不外推所有dirty意味着值改变。

强制global dirty false改变8540个权重控制记录，clean-cache原生值合同1拒；强制raw global为identity改变10260个记录，同合同1拒。两者完整旧body都保持，证明只是观察元数据消融。原旧报告缺数组先拒。按reader.final.cpp字节恢复、同package完整终态重建后重新提取四完整请求，全部缓存表与基线byte一致；默认Rig、角色、三轴CurveVector完整旧probe精确。reader身份从474a显式迁移，其余395和六库/原Playground/生产/引擎/转换器/原资产保持。

复验先build.ps1 -Label <new>并确认终态成功、package源码bytes等于reader.final.cpp，再run.ps1 -Label <new> -Request actual-before/actual-after/weights-before/weights-after；run_python.ps1 -Name check -Case <label>，以及transparent/ablation_check验证元数据限定。reader.global-false.cpp、reader.global-identity.cpp和ablate.py保留最小消融；最终restore并同package重建和完整重读。run_compat.ps1与compatibility.py核角色/三轴，default.request保持旧Rig请求。所有自动流程复用已自验固定wall/no-progress watchdog。

长期native、请求、原始读数、完整表、失败/恢复、源码、身份及manifest全部在Git目录；gzip解压rawSHA/长度精确，构建二进制仅缓存。截图0、性能验收暂停。

只关闭观测能力，writer实际执行/Equals计数、source/target完整后代local/global数学及整个Rig/Root仍由后续审计完成。
