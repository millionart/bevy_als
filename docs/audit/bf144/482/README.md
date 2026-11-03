# BF144.482 脚部写回与完整最终姿态

从 481 的 IK 后原始缓存开始，使用 473 已资格的脚部 setter 参数。源端 1,332 次脚部 global 结果与原生数据一致，其中 1,322 次 Equals 跳过，2 次零权重早退，8 次写入。两端都按骨树先解析 local 再解析 global。源端 666 帧和目标端 636 帧各 79 骨的两个空间均逐值匹配，共 205,716 个变换。源端两条 319 帧历史的结果相同，按两个捕获历史保留。

源 IR 生命周期检查以实际 visited 顺序读取。pc130 后会执行低编号的分支块，两次手臂 IK 的实际权重均为 0，权重寄存器此后没有别名写入。其余访问只读变换、曲线、世界条件或标量。目标端在 setter 处用已有 Debug 观察完整缓存。最终 global 读取发生于实际 local 输出完成后的克隆上，原 VM 的调用、寄存器、trace、local 与 481 完整对照一致。

固定右脚 setter 之后的源缓存，分别在厘米和米中调用目标的公开变换方法。两个域共用原始 dirty 标志和骨树。ball 的本地变换及父 foot 全局变换在同厘米对照中逐值保持。只换公开 global 算术时，左 ball 有 469 个 F32 平移记录不同，右 ball 有 619 个，属于 D247 向量旋转消费者。只改变厘米转米的时点时，有 2,654 个 local 平移 F32 投影改变，global F32 投影保持一致，归 D232。其余同厘米算术差和两端自产历史的差异分别列在 comparison.json。这里没有将算子组的合成差拆成未经验证的单因素数量。

最小加权帧去掉 setter 写入后，右 foot 的 local/global 与右 ball 的 global 失配。直接读未求值的 local 缓存后，源端样本有七骨失配，公开目标样本有六骨失配。源码和输入恢复后，同 target 重新编译得到完整报告原始字节一致。

目标解析器首轮未在大报告加载间隔产生进展日志，触发 watchdog。保留超时报告，分段处理后通过。原生生命周期首轮未包含尾部回跳，保留拒绝并改按实际 visited 顺序验证。持久证据及压缩原始 SHA 均在 Git 目录。

复验从项目根用 run_python.ps1 执行 facts、lifetime、prepare、current_inputs、compare、identity、finalize。运行 run_current_vm.ps1 -Label baseline -Build 生成公开历史。独立重放前可用 ablate -Case restore 从归档恢复 inputs.json 和源码，再运行 run_replay.ps1 -Label restored -Build。消融名称为 setter 与 local。压缩前后的原始字节由 compression-map.json 对照。

本项覆盖源脚部 setter 的目标骨与完整最终 pose，目标端另核完整 raw setter 缓存。包围 Rig 的最终输出传递和原 Root 反馈是下一边界。截图关闭，性能验收暂停。
