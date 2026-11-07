# BF144-D275 动画源单位系数

fresh 源读取使用正确 SKM，`raw.native.json` 与 `compressed.native.json` 各包含 Stand Pose 两个导出采样的79骨；请求、watchdog 报告、源资产身份、UE 导出公式和26个共享目标引用保存在本目录。`generated-report.json` 逐项记录改动的骨、轴、帧、源厘米值、旧新 F32 和二进制偏移。

离线生成只处理两种源模式数值相同、且目标键能由源 `FVector3f * 0.01f` 精确复算的标量；在148个此类标量中22个改变。另326个 raw/compressed 不同的标量原样保留，相关模式差异由 D224 处理。原始 glTF/bin 保持不变，独立目标副本位于 `assets/ALSRuntime/Animations/Base/`；原 GraphClip descriptor 指向该副本，Standing、Head、Layering、Locomotion 和 Overlay 等共享调用仍使用同一 descriptor。此前 .449b 的单字段克隆用于隔离差异，保留为历史证据，不宣称其他消费者已经通过。

从仓库根目录执行 `pwsh -NoProfile -File docs/audit/bf144/d275/generate.ps1` 可由 Git 跟踪的原生读取和原 glTF 重建目标副本及描述文件。`gen_ron.py` 会按相同相对路径优先选择已有的 `ALSRuntime` 副本；源数据变化时先重跑生成器。生成结束只表明数据已写入，测试、消融和视觉验收未运行。

生成器和共享 descriptor 导入规则已变化：全部条目实现后必须对全部已生成 AnimBP 及递归依赖执行全量转换，并以完整新旧产物 diff 核对和处理影响；同时运行 focused 用例、图静态审计与公共视觉 Gate。消融时让源单位投影退回 float 系数，确认 骨盆首高度反例失败；恢复源码字节、同一 target 重建并复验。以上检查均待执行。
