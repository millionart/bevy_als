# D67 同帧布娃娃退出复验

ALSExtras 的 Jump 先同步调用 `StopRagdolling`。加载角色的三条入口验证同帧 GettingUp、输入阻塞及退出请求清除。Bevy 调度图把系统集的成员关系和依赖分开保存，构建执行图时才折叠集合。旧测试从具体 System 节点起步，遗漏其上层集合的前置边。

从项目根运行 `cargo test --locked --profile play --lib --jobs 2 ragdoll_exit_`。单独移除退出链上的 `.after(crate::als::motion::locomotion_drive_system)` 后，调度断言确定失败。恢复源码字节并用同一 target 重编，九项通过。两次 stdout、watchdog 及恢复源码哈希随此记录保存。
