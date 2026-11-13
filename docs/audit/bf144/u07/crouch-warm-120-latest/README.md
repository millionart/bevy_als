# U07 同 1.2 m/s 历史的动画时钟

正式 Playground 的实际 89 行包含一次零时刻求值和 88 个普通动画步。与上轮目标对比，原有姿态、曲线、运动和 Rig 字段逐值相同。新增观察读取全局实际 delta 和前 17 个动画帧的状态机累计时间。Native 请求与旧报告见相邻 `crouch-warm-120` 目录。

从落地到 Grounded 的五个累计时刻逐值一致。已录的 `blend_alpha` 是目标判定结束的内部累计值。初次进入稳定状态后，源 PoseGait 为 1，目标为 0.9999998211860657。需要读取实际曲线混合节点与叶，才能确定差异阶段。

复跑可解压应用相邻目录的 `observer-hook.patch.gz`，把本目录 `observer.rs` 放到钩子引用路径，再用 `replay.ps1 -Directory <new-directory>` 走正式入口。取证后移除钩子与临时模块并核对源码字节。
