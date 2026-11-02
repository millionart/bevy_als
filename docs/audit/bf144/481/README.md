# BF144.481：实际 IK 三写者与全层级缓存

承接481a原始缓存与480独立三骨请求。fresh UE加载代码从直接parents建立ElementsToDirty；单parent骨树、SetTransform的clean/Equals判定、global getter和两阶段传播逐项核对。1332 source及1272 actual current每份79骨local/global原始缓存和值/dirty完整匹配，共205716个自然缓存节点；2次源weight0不进入writer，其余推导3990 source/3816 current写入、2660/2544 global重算、2660/2544新dirty标记。计数来自fresh源算法在真实输入上的重放并由完整缓存端点交叉，不冒称原生内部trace计数。

目标缓存用已有Debug格式直接只读观察current字段，不调用pose getter、不加生产API。两条319完整VM的所有旧寄存器/调用/trace/pose保持；parser覆盖全部79 BonePose且拒绝截断/非法dirty两最小反例。请求仍取480独立已资格模型，不把post缓存回填为输入。

原始off-chain后代已经global dirty，删传播可能被自然终态掩蔽。因此额外克隆一个公开before层级，先通过public getter将全局全部materialize为clean，再按同一actual三骨请求调用public setters；控制5个dirty标记/2次global重算/3次写入，完整79骨模型与public匹配。物理去传播保留最小source自然样本及其2个native twist，却使该clean控制缓存合同101失败；漏标writer local dirty使source及public缓存失败；只回退source normalizer则两个缓存端点仍精确、2个native twist失败。三个最小输入/源码/失败均保留。

source两阶段遍历适配去掉后完整原报告byte同；未访问的local求值/Equals skip实现及恒零状态再去掉，所有原始缓存/消费者值保持。实际writer条件由断言逐项确认dirty或不等，没有把未遇到的Equals skip算通过。只保该已资格单树/实际输入的最小实现，源完整代码仍在source-facts.json。首次one-pass启动与前序变更有潜在时序交叠，验收仅使用确认变更终态后重新编译的one-pass-ordered报告。

每次IK后的两twist全局从新cache独立求值，2664份全native精确。Source IR后续foot writer不在这些twist祖先链上，因此可用frame-end globals交叉；ball受后续FootRotation setter影响，未拿其帧末值冒充IK后。Ball最终、完整local及整个Rig/Root继续后续项。source global normalizer反例只扩既有D248，不新增编号、不改生产。

源码与完整输入bytes恢复、同target六库重编，controlled/focused-counts/restored全报告byte一致。大输入和报告gzip保留原bytes/SHA，长期证据全部Git；二进制仅缓存。复验从项目根先run_python.ps1 -Name ablate -Case restore恢复inputs.json，再run_replay.ps1 -Label <new> -Build；或prepare/current_inputs -Case clean-control从父证据重建。原始完整VM用run_current_vm.ps1 -Label <new> -Build，current_inputs parser读baseline/clean-control既有完整输出。源事实/身份需ALS_UE_SOURCE_ROOT与既有gitignored配置。自动流程复用已自验fixed wall/no-progress watchdog。

截图0、性能验收暂停，生产、reader、引擎、转换器、原资产与原Playground均未改。
