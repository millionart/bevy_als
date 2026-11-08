# BF144-D257 Transition 末端采样

fresh UE 源码显示，正向动态 montage 到达没有后继的 section 末端时，停止播放并将位置设为 section 末端减 `UE_KINDA_SMALL_NUMBER / 2`；ALS Transition 使用单序列、loop 1 的动态 montage。左右 Crouch Transition 的 fresh 原生时钟均确认完整长度 2.3333332538604736 秒，正向到达或越过末端后为 2.3332831859588623 秒；末端初始零步长、反向播放、零速和普通前进控制见 `clock.request.json` 与两份 native 输出。

右侧序列使用同一 SKM 和 compressed pose，在源末端位置与旧配置截断位置采样；`pose-comparison.json` 将源厘米位置换为米并投 F32 后，79 骨中 14 骨平移、73 骨旋转发生变化，scale 和曲线无变化。重新生成对照可在项目根运行 `pwsh -NoProfile -File docs/audit/bf144/d257/compare.ps1`；原始样本、资产身份和 watchdog 报告均保留在本目录。轴映射仅是分量重排及变号，不改变此处相等性统计。

共享 Transition 时钟在正向实际到达末端后保留源偏移，零步长及反向路径保持各自行为；Playground 的停止完成判定和现有时钟夹具同步使用该末端。D118 的逐步时钟和 D121 的配置时长精度仍是独立条目。旧 `.333/.387` 报告当前缺失，本轮证据不依赖其摘要。

focused 测试、移除末端偏移的消融、源码字节恢复后同启动器 target 重编、图静态审计及 `startup`/`standing_stop`/`crouching_actions` 公共视觉 Gate，均待全部静态修复及 Bevy/依赖迁移后执行，目前尚未运行。
