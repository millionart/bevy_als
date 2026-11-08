# BF144-D88 清醒刚体的显式速度唤醒

fresh PA输出与D65a原生记录逐字节相同：hand_l与lowerarm_l的线速度休眠阈值为1cm/s、角速度阈值为0.05rad/s、计数阈值为4；当前全局partial island sleep关闭。源速度写入会刷新平滑速度；只要线速度或角速度任一分量超过`UE_KINDA_SMALL_NUMBER`，即使body已清醒也重置计数，连接岛还禁止当前物理帧休眠。线速度容差需从cm/s转为m/s，角速度保留rad/s；离线执行 `python3 tools/import/import_sleep_velocity_tolerance.py --facts docs/audit/bf144/d88/source-facts.json --output src/avian_ext/sleep_velocity_tolerance.rs` 可重建目标值。

目标以Avian已有的上一物理帧结束tick区分外部速度写入和solver自身回写；仅对动态源body更新平滑值与显式唤醒标志，孤立body从本帧重新累计，连接岛本帧保持清醒并在后续重新累计。已写实际加载hand、独立/连接岛、线/角速度和分量近零边界的focused对照。测试、消融、partial sleep与Playground待统一Gate；外力平滑初始化归D89。

D89的fresh源码闭包发现项目接触冲量会直接改写Avian速度、CMC repulsion还会在NarrowPhase之后设速。两者现分别记录冲量来源并将设速捕获排在solver body准备之前；已补冲量与设速前后顺序及同帧写入对照，实际执行仍待统一Gate。证据见`docs/audit/bf144/d89/`。
