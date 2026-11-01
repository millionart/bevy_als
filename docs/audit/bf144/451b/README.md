# BF144.451b 全318帧Head四输入域矩阵

四域为cc（current Base/current Additive）、sc（source Base/current Additive）、cs（current Base/source Additive）、ss（source Base/source Additive）。全部318个共同正步长帧、每域79个source骨，native和current各1272输出/100488骨行。6批原生请求各≤240样本，复用已有compiled Head API；每批有真实样本进展和180/90秒watchdog，不修改reader或生产。

两个端点全量校准：native ss全部318帧的F64完整Pose/curve包精确actual Source RigSource；current cc使用实际捕获Skeleton重建，全部318帧精确目标actual Head输出。当前F32输入lift为源cm/坐标只保留当前信息，不冒称源自产。native四域固定使用source曲线；只分类pose数据，curve差不据此关闭。native full-TQS边界将缺省additive scale显式映射语义identity0，current Pose仍保None；额外SKM_Als维持current且不计source79骨。

| 比较 | T差骨行 | Q符号对齐后差骨行 | S差骨行 |
|---|---:|---:|---:|
| 相同current输入，native cc→current cc | 0 | 23032 | 0 |
| source数据经当前投影/公开核→actual source | 403 | 22719 | 0 |
| native ss→actual source | 0 | 0 | 0 |

其余完整四域结果、两条data因素的交互边存`analysis-restored.json`与frame table。影响数不可相加。T/S在当前输入域没有组合算子余差，实际差来自输入；这只限本cohort，不泛化所有数据。Q的大量same-input残差明确留组合数值管线，不能都归D224/D235或用首XYZ恢复掩盖。source数据在current域的403T/22719Q仍含投影与算子，不冒称仅数据恢复。

去source Base/去source Additive替换分别被完整输入合同拒；源码SHA/字节恢复、同target六库重编后1272 current输出、analysis及全frame table字节精确。四类缺域/重复域/错alpha/错骨映射反例拒。396现行身份、六库、原main/reader/原资产保持；复用.a原场景functional Gate，无新增角色运行。11大矩阵文件mtime=0无损gzip并核解压bytes/SHA，原始输入、请求、native/当前输出、脚本与manifest均Git管理。

复验先`pwsh docs/audit/bf144/451b/run_replay.ps1 -Label new-check -Build`，再`run_python.ps1 -Name analyze -Case new-check`。请求和replay-inputs保持直接可读；native/输出采用gzip自动读取。无新D、272未修，完整Root与明确Q边界继续；截图0、性能暂停、GPU周边排除。
