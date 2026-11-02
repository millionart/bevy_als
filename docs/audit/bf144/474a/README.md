# BF144.474a：单元调用边界的原生 local dirty

474 的 vectorized global→local 算法资格已有 694/696 精确结果；零权重两行保留输入 local，必须用实际 native cache 状态区分，不能按最终输出反推或写 case ID 分支。本前置复用已有 opt-in unit_boundary，只给有效 typed item 增加一个布尔 current_local_dirty_before_global_read。公开 const GetDirtyState().IsDirty(CurrentLocal) 在原 FunctionPtr 执行后、既有 global getter 之前读取，不调用 local getter，不清 dirty，不增加请求开关、运行时状态或重实现源算法。

SetRotation pc100/130 的真实 Item 在 2×319 帧提供 1272 个 dirty=true；5×7 权重控制提供 58 个 true、2 个 false。对应 source frame 的完整旧执行/working/pose/层级及 descriptor 恢复均保持。该受控 cohort 的 dirty 行恰好都改变 local，clean 行逐值保留输入；检查器只对这批既定输入核该关系，不把 dirty 泛化为值一定改变，不在本项宣称数学等价。

原 FootRotation 边界追加 2544 个 typed bool 后，删除这一新增字段的完整旧 probe 精确。Getter 固定 false、true 分别产生 58、2 个错误 flag，两个检查均失败，其他全部原生数据保持；源码按 reader.final.cpp 字节恢复并在同 package 完整构建后，完整 flag/global/local 表必须恢复。默认 Rig、角色和三轴 CurveVector 在最终 package 下重新核对。

复验从项目根运行本目录 build.ps1 -Label <label>，确认该进程终态成功及 project/package 源码字节相同后，再运行 run.ps1 -Label <new>（2×319）或 -Request weights.request（5×7）。check.py/其 watchdog wrapper 验证 metadata、已选 cohort 与完整旧 body；check_legacy.py 核原边界的 additive 兼容。reader.flag-false.cpp / reader.flag-true.cpp 是消融源码，结束后必须按 final 字节恢复、同 package 重编并重读。run_compat.ps1 和 compatibility.py 核默认角色、三轴 CurveVector；run.ps1 -Request default 核原 Rig。

角色兼容入口首次误用 AnimBP，按原生类型检查退出 7；失败单独保留，不纳入兼容性结论。依据旧报告的 Character Blueprint 资产修正入口后重新提取。

所有长期读数、请求、失败/恢复、原生源码事实与身份清单在本目录并随条目提交；大报告无损 gzip，解压原 SHA 验证。395 其余身份、六库、生产/引擎/转换器/源资产保持；reader 从 469a 显式迁移。截图 0，性能暂停，父 474 的局部数学/实际输入组合仍待此前置提交后继续。
