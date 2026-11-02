# BF144.473：SetRotation global、近似早退与部分权重

fresh CR 原生 IR 核对 SetRotation 的 Item/Value 直接使用 FootRotation 输出。98→100、128→130 间无 pose writer，7 个 setter 参数无调用后 alias 写入；后续右腿与双臂 writer 不在当前足的祖先闭包，因此原生单元边界 foot global 是合法 pre-pose，帧末 foot global 是合法 post-pose。源 GetTransform 先读 current global，使 Equals 检查所需 global cache 有效。这里只核 global 值，不声称已观测 pose version/完整 local 与 descendant dirty 生命周期。

保持原 source 世界和前六帧，只改第七帧叶 pose 的 FootLeftIk/FootRightIk 曲线，原生 work 中实际 setter 权重确为 0、.25、.5、.75、1；不是把 requested alpha 当读数。636 实际与 60 控制共 696 次源写入，按 fresh SetRotation→SetTransform→LerpTransform→Slerp→ARM64 normalizer→Hierarchy Equals 重放，所有 T/Q/S 逐值原生精确。636 实际与 52 控制保留旧 global，8 控制写入，其中 6 次部分权重、2 次全权重。T/S 均不变，初始姿态保持。

共同输入 current public setter 的 5 次 double 差均在部分权重，F32 差 0。单用 source Slerp 仍差 2 次，单用 source normalizer 仍差 3 次，二者同用全恢复；只扩既有 D250/D248 的 consumer，不登记重复差异。2784 组合保留完整 weighted/result，不按差异数量相加推根因。

原/trace 控制两条 319 连续 public VM 重新执行，完整 register/local/call/trace 历史与 464 保存记录逐值相同；新增只读 after-global 观察显示 1272 次真实 setter 均保留 before。独立 public writer 逐值重放全部 cached global。10176 个 pre-pose/Value/current-source math 组合中，真实原/trace global F32 差 470/121，换算法或 Value 不恢复，换 source pre-pose 全恢复。这是输入资格与 skip 语义的证据，更早的 IK pose 生产根因仍未闭合。

自写 ±quat 近似 helper 删除，复用现有 public 比较后，696 次源、1272 current 端点及全部矩阵/阶段报告字节相同。物理去 Equals 在第一个实际源端点失败；物理去部分权重后只有 690/696 源端点匹配，两者 exit 101。最终按 replay.rs.final 源码字节恢复，在同一 target、同六库重编；完整报告与基线及简化结果逐字节相同。当前 F32 合同负检查稳定失败。

从项目根复验：run_python.ps1 -Name prepare 重建输入/lifetime/旧历史兼容，支持已压缩原生与 VM 证据；run_replay.ps1 -Label <new> -Build 运行完整源/public 矩阵；run_python.ps1 -Name compare 核对已封存基线，-Case current-f32-contract 重现当前失败。current_vm.rs 与 run_current_vm.ps1 可重放两条完整 VM 并读每次 setter 后 cached global。替换 replay.no-equals.rs 或 replay.no-weight.rs 可重现消融，最后必须按 final 字节恢复并同 target 重编。facts/identity 使用本地配置与 ALS_UE_SOURCE_ROOT；所有自动流程复用仓库已 self-test 的固定 wall/no-progress watchdog。单独 Rust 诊断不充当新 Playground 运行验收。

summary、原生请求/读数、完整输入与结果、失败/恢复、source facts、身份和 manifest 均在 Git 管理的本目录。大原生/历史/矩阵无损 gzip，并验证解压原字节 SHA。构建二进制只作缓存。396 身份含显式 469a reader、六库及真实 Playground 源码保持；无生产/reader/引擎/转换器/原资产修改，无新增差异，累计 272，截图 0，性能暂停。

下一边界为 global→local/descendant 写回与更早真实 IK pose 生产者；不称完整 Rig/Root 等价。
