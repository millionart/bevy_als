# BF144.444 当前物理反馈到动画输入

所有长期证据在本 Git 目录。大型原生阶段投影无损保存为 `source-animation-phases.json.gz`，解压字节/SHA见 `projection-compression.json`。`prepare.py` 从 `.443a` 的原 Playground 观察器生成独立 main，仅增加只读观察，保留原地形/环境/角色/相机与驱动。构建和运行由现有 watchdog 约束；截图 0，性能暂停，GPU pass 仍按用户排除周边而不验收。

复验数值：`pwsh docs/audit/bf144/444/run_python.ps1 -Name check -Case self-test`，再分别 `-Name check -Case baseline` / `restored`。Rust 逐值重放用 `pwsh docs/audit/bf144/444/run_replay.ps1 -Label restored -Build`。身份检查需要从 gitignored 本机配置与环境变量读取源根目录，脚本不保存本机路径。重跑真实场景应使用新的 Label，不能覆盖封存目录。

fresh Character/AnimationInstance 调用链及 target 调度明确：物理结束后，速度快照及 locomotion 同步先完成，整个 Inputs 阶段完成后读取 provider，Final 后再次读取。240 ready 的物理后速度与 motion/provider 完全相同；局部水平速度长度、连续 F32 加速度及 pending-update guard 按实际字段重放。旧 `.443a` 三阶段完整数组未变。既有 D151/D256 的世界速度/厘米 float 根因保留，本批不另编号。

初次目标查询是 Absolute(0)，之后为实际动画 Delta；ready 240 开始前已更新 79 次，source 原生240从0开始、每步主实例 update/eval 各加1。Root pose timestamp/elapsed 是图最终 pose 的时间，不是全部85个叶播放器的时钟。本批保留完整 native 叶字段，但尚未对当前 target 叶时钟与相同初始化历史作对照；不能从入口时钟一致推断完整 Root/播放器等价。源反射 state 文本仅六位小数，不能当作精确数值 oracle。

消融移除两个观察注册使两数组为空、合同失败；只移除模型首 Absolute(0) 分支使最小原生 API trace 反例退出101。源码 SHA 恢复、同 `target_game` 六库重编后，完整阶段数组和重放 JSON 精确恢复，公共动画 functional-only baseline/restored 通过。初次导出路径/depfile 选择失败与负例 stderr 已持久化。初次正 replay 的进程元报告在消融调用时被覆盖，负报告已另存；保留的正数值 JSON 与恢复后的独立正进程报告才是最终证明。
