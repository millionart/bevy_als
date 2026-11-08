# BF144-D243 首个普通动画时钟

本地 `USkeletalMeshComponent::InitAnim` 在注册时先执行零步长动画与骨刷新；它把 `LastPoseTickFrame` 设为零，非零 engine frame 的首个正常 world tick 仍会运行动画。`FAnimInstanceProxy` 的实例级线程更新有同帧防重，根动画节点更新则不在该防重分支内。fresh `first-tick.request.json` 使用 engine frame 17、150 cm 空中出生和 11 个 1/30 秒 world 步；原生结果中注册状态为 Grounded/elapsed 0，首普通帧变为 Fall/elapsed 0.03333333507180214，两个实际 Fall 叶播放器也各推进同样的时间；10 个 Fall 步与第 11 个 Land 步完整保存在 witness 和原生结果。请求、读数、源码摘录和哈希均已保留，压缩结果已逐字节解压核对。

目标首个 gated 图求值保持零步长初始化；成功取得 pose 且仍在播放时，同次入口再按原 delta 执行一次正常图查询，普通刷新标记在第二次查询前恢复。失败、尚未加载、暂停和单帧播放保持各自状态，不提前吞掉查询。根图及链接图测试代码覆盖 gated/ungated、显式 delta/时间资源、禁用 gate 与首帧位置，后续连续帧和事件沿用现有测试。

该实现只处理图时钟，零查询出生输入仍按 D244 修复，pending 生命周期仍属 D149；未声称 pose 或曲线已与源相等。旧 `.297/.298` target 原始报告不在仓库；本次 focused 测试、消融、转换图审计和真实 Playground Gate 均待全部静态修复与 Bevy/依赖升级后执行。
