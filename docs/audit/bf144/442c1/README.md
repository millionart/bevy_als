# BF144.442c1：真实 Playground 视点生命周期

复用原 `manual_playground`、六个 `target_game/play` 运行库和 .442 的已证观察阶段。首次 ready 前被动记录实际相机状态、`GlobalTransform` added、published/raw yaw、input blocking、实际输入和 Tnua basis；ready 后按原输入路径移动，仅第 20、21 帧临时关闭所属 render camera，22 帧起恢复。相机算法、场景、地形、角色、图和生产代码不改。

baseline 的 81 个自然启动快照中有 2 个没有 published view，实际输入和 desired motion 都为零、blocked 为 false。该样本没有证明自然启动时发生非零冷输入。ready 的 240 帧实际 basis 全等实际 published/raw yaw 和输入的公开公式；受控第 20、21 帧用 `[0,1]` 输入消费 raw yaw，随后恢复 published view。raw yaw 为约 −150°，实际 actor yaw 是另一观测字段；两者不视为同一角，也不将该受控 camera-off 条件等同于 source cache time 0。

首轮检测器要求“后采样相机 eligible 则此前必已发布”，被自然启动第二帧反驳。实际编译依赖的 `DetectChanges::is_added` 使用查询 system 自己的 last_run，publisher 又先于 consumer/采样器执行，故后读资格不能反推前阶段。删除该无 source 支持的断言，保留原始读数与失败 JSON。`tick-source-facts.json` 记录实际依赖源码和 SHA；本项没有确定该具体间隔的唯一根因，也不凭它复开生产或旧 .442a。

五个 focused 负例验证视点、方向、blocking 和零输入合同。消融去掉 camera-off 控制以及 cold 采样注册，分别失去两类信号，检测器拒绝。恢复按 `final-sources.json` 的源码字节 SHA，使用同一 target 库重编并再跑原场景；两个最小回退帧的输入、blocking、published/raw/consumed yaw 和实际方向须精确恢复。自然加载快照数量可变，不声称整个场景输出字节相同。

公共 `validate_animation_visual_evidence.py --functional-only --min-screenshots 0` 分别审计 baseline/restored 的真实报告。原始报告的性能失败原样保留，性能验收继续暂停。每个 runtime 300 秒总上限、90 秒无参数进展上限，进展来自真实加载状态或已记录帧数；沿用已有 watchdog 自测过的实现。

复现于本提交检出树中，使用新 Label 保留封存目录：

```sh
python3 docs/audit/bf144/442c1/check.py self-test
pwsh -NoProfile -File docs/audit/bf144/442c1/compile.ps1
pwsh -NoProfile -File docs/audit/bf144/442c1/run.ps1 -Label replay
pwsh -NoProfile -File docs/audit/bf144/442c1/functional.ps1 -Label replay
python3 docs/audit/bf144/442c1/check.py replay
```

`ablate.py ablated|restore` 控制变体；`controls.py` 检查两项信号消失，`finalize.py` 合并原始数据、恢复与公共功能 Gate。`identity.py` 核396现行身份、六库和原 main 未变，reader 身份明确承接 .442c0。证据全部在本 Git 目录；可重建二进制和临时日志使用忽略目录。未执行 source warm-cache 原生生命周期，未证明自然非零 cold 输入，也没有新增差异编号。截图 0。
