# BF144.447d 实际View producer与量化阶段

目标原Playground319帧的局部TQS→实际GlobalTransform forward→公开yaw helper→f32度数→相对角，逐值重放全部实际View.YawAngle。源318帧的typed double View/Locomotion.RotationWorldSpace先差分、后cast float/Unwind也逐值等于原生实际YawAngle。原始报告与当前生产身份沿 `.447b`、`.446f`，没有重写源资产或请求。

使用已有GeometryQueryProbe，把目标真实raw view radians与相同可表示body degrees交给UE公开RadiansToDegrees(double)/Unwind算子，319份原生结果全部匹配晚cast控制，全部与目标提前cast结果不同。该共同标量域冻结body，仅分离view换算阶段；不声称实际source/target body quaternion或head初态一致。坐标角原点在相对量中消去，首Look控制为45度，实际目标44.999996185302734度。`.447c` 已证明仅恢复这个View字段就恢复当前首Head/Look输出。

这与D237已证的同raw radians在差分前过早量化为f32度数是同一根因。这里只扩实际Head/Look消费者，不新增编号，也不把其它body/视角历史余差归并。D258只管半转规范表示，本项普通45度未触发它。旧 `.67` 的1e−4度合同漏检微量差，有限复开后以实际消费和逐值控制补齐审计边界。

删除晚cast控制、合并为现行结果后，校验在native值比较处失败；fixture源码SHA/字节恢复后，同target六库重编，完整replay/check报告byte精确恢复。缺行/错输入/错native结果三个反例拒绝。旧运行脚本对ablated标签的重命名导致一次缺文件失败，已删除这条无用分支；该次不作消融通过，实际反例为 `check-factor-ablated.report.json`。396身份/六库/原main保持，final package源码匹配当前reader，reader未修改。原 `.447b` 真场景功能Gate可复用，本项没有生产运行改动。

复验使用新label：`pwsh docs/audit/bf144/447d/run_replay.ps1 -Label new-check -Build`；原生请求由相同fixture生成，调用 `run_native.ps1 -Label new-check`。正式证据全在Git目录，构建target仅用于身份核对。截图0、性能暂停；完整Root及其它实际producer余差继续保留。
