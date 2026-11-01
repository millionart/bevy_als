# BF144.443b：完整当前物理与竖直floor写者

本项审计原Playground同一240输入，生产/引擎/图/资产不改。新增只读原forward/down shape query、Physics结束到floor处理前的body/floor、PreSubstep全部写者之后的积分系数与solver初速、motor实际acc/gravity。父.443与整体ALS仍保留未覆盖边界，不能将本项当作所有源/目标反馈等价。

94份非零位移/free-forward/有效edge与walkable down query解释89次Update前置Y改写，89次距离换source float厘米均在[1.9,2.4]cm内。Source AdjustFloorHeight带内不移动，target单步事务却向固定0.019+0.005m clearance上抬。首20号在净力系数0、竖初速0时由0.923999905586升到0.924395799637，raw/final world均保留，独立于D31弹簧。固定无query误差的center/distance为0.9215/0.0215f32，目标仍写0.924000024796而source保持center，独立于D125。登记D273/P3，未修生产。

从实际motor加速度、可选Tnua重力覆盖及Avian默认gravity/scale，经子步dt、full-frame ratio得到全部实际系数；damping→increment→6次积分→一次world writeback，240完整XYZ位置/速度精确。78次后置出带floor修正通过相同public find_ground_floor、实际TransformHelper即时scale和公开Dir3归一/length逐值复现。旧cached GlobalTransform会少数帧给不同scale/半径并使29号差1ULP，失败保留，不放宽精度；source-target世界scale producer尚待父项后继核对。

第一次coeff观察放在未完成全部PreSubstep写者处发生调度冲突，watchdog按无进展终止；一次复算启动过早缺文件，明确无效。随后motor观察误把可选gravity组件写成必需，按真实API改正。failed报告和纠正均保留。所有重复运行均等待旧进程terminal，label独立；无ambiguous_with掩盖冲突、无修改引擎或阈值。

当前环境的GPU TAA pass在instrumented与未改动公开startup对照中均5秒超时，公开animation functional报告仍通过。按用户明确排除周边，复制main仅将RenderEffectsProbe.directory设None，保留组件、原场景/角色/相机/渲染插件及animation报告；生产入口、公开runner、5秒阈值、材质与GPU配置不改。本项**不宣称GPU pass验收通过**，动画/运动functional-only仍分别核baseline/restored，截图0、性能暂停。

消融只换source带内不抬升决策，原89个actual transaction反例须退出101；删query/raw/coefficient观察丢对应信号，阶段合同拒绝。源码字节从Git内*.final恢复、SHA通过同target重编后，完整replay JSON byte精确、完整runtime数组精确恢复。`finalize.py`合并这些证据。全部长期证据Git跟踪，早期失败完整保留。

```sh
python3 docs/audit/bf144/443b/check.py self-test
pwsh -NoProfile -File docs/audit/bf144/443b/run_matrix.ps1 -Label replay-new-build -Build
pwsh -NoProfile -File docs/audit/bf144/443b/run_matrix.ps1 -Label replay-new
pwsh -NoProfile -File docs/audit/bf144/443b/run_python.ps1 -Name finalize
```

Scene/runtime新运行用`run.ps1 -Label <new-label>`，不覆盖封存目录；300s总上限与90s无参数进展上限沿用原watchdog。范围和失败边界见CHECKLIST当前条目，累计270差异（116P2/154P3）。
