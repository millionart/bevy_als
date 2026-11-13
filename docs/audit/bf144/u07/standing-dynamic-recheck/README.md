# Standing Dynamic 左右脚转场

源 ALS 在 AllowTransitions 为满权重时，比较两脚实际锁点与目标位置，锁权重有效且偏距大于 8 cm 才可选。旧正式夹具向前偏移 10 cm，首帧应选左侧，但用例等待右侧。改为等量向后，两侧通过实际曲线与比较选中各自蒙太奇、解锁并回锁。加载回归在落地后用原图重新初始化，等待真实 AllowTransitions 满权重再移动。

原生根图的 Transition 槽位于 Grounded 图之后，Locomotion 之前。反向输入已通过左右选片和回锁时，旧视觉边界仍将四个稳定 Idle 上游节点判为失败。恢复版本的活动姿态从该源槽验到下游 Locomotion，原有脚锁、动画生命周期、前 Rig 与最终姿态条件继续通过。
