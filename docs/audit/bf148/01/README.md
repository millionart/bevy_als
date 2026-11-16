# BF148.01 连线输入

本地 UE 源码版本见 [native.json](native.json)。`ExposedValueHandler.cpp` 42–47、71–76 先执行编译得到的输入处理器。`AnimNode_SequencePlayer.cpp` 53–67、89–103 在初始化和更新阶段读取输入处理后的起点、速率等属性。`AnimNode_SequenceEvaluator.cpp` 24、35–56 先执行输入再取时间。`AnimNode_BlendListBase.cpp` 111–126、`AnimNode_TwoWayBlend.cpp` 41、`AnimNode_ApplyAdditive.cpp` 44、`AnimNode_ApplyMeshSpaceAdditive.cpp` 49、`AnimNode_MultiWayBlend.cpp` 98 及 `AnimNode_ModifyCurve.cpp` 173 有对应边界。源原生记录覆盖 588 个图，含空接的 Standing PlayRate、Locomotion ExplicitFrame 与已接的 Land Movement Additive Alpha。

目标共用 `optional_data_back` 检查本节点 target edge。空接保留现有字段、全局输入和事件默认路径。存在边时直接返回图求值的结果，由各节点验证消费类型。横向检索覆盖播放器、求值器、标量表达式、混合激活与时长、alpha、曲线值、臀部方向和可选事件队列。Bool Blend 的预览与正常更新共用输入读取。标量比较按实际运算读取类型，避免先把 Bool 连线当 F32 读。

同一 `target_game`、`play` profile 的最小 Rust 用例固定两个节点各自的空接、有效输入、缺值、类型错和上游错误。首轮夹具漏设播放器针脚配置，节点实际读取 `Play Rate` 而边名是 `PlayRate`，前后两次夹具失配保留在 `miswired-fixture-*`。纠正后显式使用生成图的 `PlayRate`，消融仅撤掉读取器对空接和求值错误的区分。按原字节恢复后同 target 重编。公共图报告验证程序保留原始 GraphError 信息。

[作者性清单](../../../../tools/animgraph_authoring/bf148_runtime_inputs.authoring.json) 核对两个代表图的 16 个节点、16 条边、位置信息和 IO，以及项目资产边界、ALS/ALSCamera 运行图位置和编辑器记录。全树位置检查基线包含 `Validation/Graphs/{actions,air,base_locomotion}.animgraph.ron` 三个既有重叠，相关 RON 在本项没有差异。

`native.json` 记录可复跑的 reader 参数、每份原始长度与 SHA-256。可用 `gzip -dc <record>.native.json.gz | shasum -a 256` 复验源记录。测试入口为 `cargo test --locked --profile play --jobs 2 --lib connected_sequence_inputs_propagate_errors_before_using_defaults`，使用 `CARGO_TARGET_DIR=target_game`。

消融判定见 `ablation.stdout.log.gz`，恢复用例见 `focused-restored.stdout.log.gz`。最终 2157 项单元测试、37 项集成测试通过。同一启动器二进制的 startup、locomotion、camera 功能 Gate 通过，均无图错误。各报告原始长度和 SHA-256 见 [runtime.json](runtime.json)，可将 `report.json.gz` 解压后用 `tools/validate_animation_visual_evidence.py --report <file> --min-screenshots 0 --check --functional-only` 重验。
