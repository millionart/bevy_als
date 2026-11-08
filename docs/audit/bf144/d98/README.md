# BF144-D98 子碰撞体与预测包围盒时序

fresh PA的head质量和原生shape与D65a受跟踪记录逐字节相同。源在积分后更新body及shape的世界bounds，再进行碰撞检测；目标则在solver写回后根据子collider姿态和速度构造下一帧swept AABB。原目标同步子collider的系统直到`PhysicsStepSystems::Last`才运行，晚于该AABB计算。

目标复用原同步系统，将其放在`PhysicsStepSystems::Finalize`：该阶段已完成solver写回，Avian的动态AABB更新又明确排在此阶段之后。已写加载head中心球与静态box的30/60Hz、单/六子步连续运动夹具，检查首帧AABB、次帧阻挡和未到达障碍控制。

测试、消融、加速/旋转轨迹与Playground待统一Gate；本项不声称所有碰撞几何生成差异已消除。
