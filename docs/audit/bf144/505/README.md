# BF144.505 VelocityBlend 离地与返回

源 `AB_Als_Grounded` 两个原生绑定按进入相关性、再刷新顺序执行。Locomotion 的 Grounded 和 Land Movement 均可消费 Grounded 缓存。原生角色从 440 cm 下落，在第 25 帧进入 Land Movement，权重等于从当帧本地速度重新计算的四方向目标。随后 PostLocomotion 播放 Roll，Grounded 实例更新计数在第 27–50 帧停留，权重保持。第 51 帧 Grounded 恢复更新并再次直接写入当帧目标。

Bevy 观察沿用可见 Playground 真实入口。按住移动键，在序号 34 触发跳跃，序号 34–58 的 Air 更新跳过 Grounded 两节点。序号 59 落地，两个节点同查询恢复，原实例直接采用新权重。60 起连续刷新使用阻尼。消融仅在已离地后清除 InitializeGrounded 节点唯一的写入动作，保留同一节点、图连接与玩法输入。落地读数从当帧方向目标改为同一输入的阻尼候选。还原源码并重编后，完整观察参数报告与基线逐字节相同。

`verify.py` 重验原生图、原生读数、三次可见运行和解压哈希。源端高处落地触发 Roll，目标样本为平地跳跃。高处落地触发 Roll 后的目标更新边界留待用同类落地对照。
