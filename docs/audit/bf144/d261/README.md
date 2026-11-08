# BF144-D261 限位与最终旋转的归一化阶段

fresh ALS 源码在足旋转限位后直接保存 `FQuat{OffsetRotation} * LockRotationWorldSpace`，最终旋转混合后才单独 Normalize。本项移除目标限位末的额外 Normalize，保留最终 Rig 旋转的归一化；Atan 与角度换算仍分别属于 D259 和 D260。

原 `target/bf144/421` 的消费者及对照文件当前缺失，不能重验旧摘要中的逐帧数字。`source-facts.json` 引用 D262a 新采的受控 80 度原生请求、响应和 reader 报告，并保存原生 raw 四元数；由该 raw 值复算的长度平方为 1.000000064686079。这里的长度平方是派生值，原生输出及请求保留在被引用的 Git 文件中。

已补生产限位到最终组件旋转的 focused 用例，分别断言世界缓存保留非单位长度和最终旋转归一化。全部静态修复与 Bevy/依赖迁移完成后，运行用例、重新引入额外 Normalize 的消融及源码字节恢复后的同 target 重编，并执行图静态审计和公共视觉 Gate；这些步骤均尚未执行。
