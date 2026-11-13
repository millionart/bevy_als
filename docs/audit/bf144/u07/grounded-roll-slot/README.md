# 翻滚退出播放槽

原生 Grounded 资产的 `PlayRollToGroundedTransitionAnimation` 以 0.2 秒起点、1.5 倍速、0.2 秒淡入淡出请求 Transition。主 AnimGraph 的 `Slot 'Transition' / Group 'Grounded'` 消费该蒙太奇。源资产读数保存于本目录。

旧目标报告在父状态机结束过渡时才开始采样。此时蒙太奇已到 1.0875 秒，错过 0.8 秒。旧报告保存在 `../stationary-floor-support/grounded/`。新观察随源通知捕获同一蒙太奇实例的播放槽，继续核对实际退出和稳定的 Standing。恢复旧观察条件会拒绝活动父机的定向反例。

新功能报告的 144 个槽输出均存在。最接近 0.8 秒的实际采样相差 0.003936 秒，终态为 Standing、无父机过渡。
