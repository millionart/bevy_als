# BF144.476a：原生IK输入/输出边界与时点契约

476 的首轮已取得 2×319 原始实际帧及 5×7 权重控制的前后边界，全部原生执行、pose、work、层级与 descriptor 恢复保持旧记录。Source CalculatePoleVector 的 readonly 单元后、IK 前没有 pose writer；source IK 后的 calf 与 foot 分别逐值匹配 frame-end/FootRotation consumer。before/after 分别是来源函数的真实 callback 时点，不把最终脚旋转倒灌成 IK 输入。

1332 个腿调用有 29304 个 source pin 读数，ordinary/hidden 引用均按真实 register/slice/path 检查后续写入；hidden 缓存只声明 after-call 状态，不伪造 before。source 初始三骨、无效 PoleSpace、debug=false、propagate=true、两显式长度=0 均直接读取。零权重两行在 source 长度/求解前退出，不能当已计算长度；其余 1330 次三骨发生改变。Effector T 与 468 独立 Location 预测、Q 与 469 原生 target-rotation 输入逐值交叉一致。Source 长度、位置、旋转与 partial 数学不在本项通过范围。

新增 current 观察在克隆 hierarchy 上读 IK 后 global；两条完整旧 VM 的全部输出/寄存器/pose/call/trace 保持。共享函数还在初始帧调用零权重手臂，且带第四个有效 PoleSpace key：原先所有 IK 都只有三份 before snapshot、初始帧无 IK 的诊断假设先被真实数据拒绝；已按实际 executed instructions 和输入 key 修正。2548 个 current 观察中 1272 是实际腿调用，其余不充作 active arm solver 验收。

把 current post-pose 当作 before 仍重现本满权重 cohort 的全部 1272 结果，是幂等无反例，已保留且不作正确输入时点证据。独立原生 temporal contract 按实际函数、pc、阶段、typed input 参数检查，1332 个合法 pre 被接受，1332 个 post-as-pre 被拒绝。最终输出相等不能替代实际消费入口身份。初版 pole 清零先被 arguments echo 检查拒绝，尚未证明 pose 必然失败；它不纳入 public 参数必要性 Gate，父476继续单独核真实输出反例与源码同target恢复。

所有原始 native、请求、脚本、完整data表与上下游读数在本Git目录；大报告无损gzip并核解压原SHA。396含474a reader/六库不变。Source/生产/reader/引擎/转换器/原始资产不改，截图0、性能暂停。476a只封存原生事实与时点Gate；父476的public完整恢复、有效输入消融和聚合仍开。
