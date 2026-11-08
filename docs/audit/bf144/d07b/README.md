# BF144-D07b 空地交接的输入缓存

本地 UE `PhysFalling` 临时使用 air-control 加速度并在 `CalcVelocity` 后恢复原输入 Acceleration；落地后 `ProcessLanded → StartNewPhysics → PhysWalking` 在余时及迭代预算允许时刷新地面曲线，步行余步仍消费本帧原输入。余时不足或迭代耗尽时不会调用 `PhysWalking`，也不刷新该值。

fresh 非零引擎时钟的真实角色落地轨迹复用 `.575` 正向输入：q58 从 Falling 落为 Walking，输入 Acceleration 仍为 2000 cm/s²，而落地后 Walking 曲线发布 750 cm/s²；q59 输入更新为 750。q54–59 的运动前后和 actor transform 与封存源记录逐值一致。首次用默认 reader 的输出缺平台/碰撞事件字段且一直 Walking，明确标为不合格并保留；重采采用 Git 跟踪的 `.572` 诊断 reader 构造流程，当前 package 与当次封存的二进制/源码哈希一致。两次请求、原始读数、报告、源事实与身份均在本目录，压缩结果逐字节核对。

目标在落地后的步行余步继续使用本帧输入缓存，并仅在 Walking 真正执行且 gait 设置存在时，把新的地面动力曲线写回角色组件；Walking→Falling→Walking 同帧也保留最后一次实际刷新。已写不同 fresh 曲线下共同缓存、耗尽迭代不刷新、真实角色组件发布及下一帧缓存控制；均尚未运行。D254 的世界位置量化和落地余时几何仍独立。测试、消融、图审计和公共 Playground Gate 按用户顺序待全部静态修复与 Bevy/依赖升级后执行。
