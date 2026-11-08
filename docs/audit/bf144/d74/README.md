# BF144-D74 mesh近零时停止Ragdoll模拟

fresh UE `OnUpdateTransform` 在 `p.UpdateRigidBodyScaling` 默认开启时调用 `UpdateRigidBodyScaling`。该函数检查相对scale的uniform及逐轴1e-4近零/变化门槛，跨到零时逐body停止模拟；`SetInstanceSimulatePhysics(false)` 将body变为kinematic并清除物理混合权重。fresh PA读取与D65a受Git跟踪记录逐字节相同，本目录保留请求、报告、源码片段及身份；旧缓存反例不可用。

目标为已绑定player记录相对scale、rig身份和停用状态，在物理交接前停用刚体并关闭动态/回读输入；world pause不阻止检测，非均匀scale不触发。新fixture覆盖0、1e-4、2e-4、非均匀scale及暂停后的实际body模式。零→非零的body/constraint重建及速度重置属于既有D116，未声称已完成；动画回读和相邻控制仍需统一Gate，测试、状态消融和真实Playground均未执行。
