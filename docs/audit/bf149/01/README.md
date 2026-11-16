# BF149.01 Control Rig 提取

由本地 UE reader 对 `CR_Als` 重新提取 RigVM，再用现有编译器核对运行时程序。两端的 Construction 和 Forwards Solve 入口、全部 45 条单位调用及 43 个曲线名一致。原生闭包含 10 个图、172 个节点和 165 条 VM 指令。原始报告、请求及哈希见 [source-contract.json](source-contract.json)。

七个模块和两个共享数学原语的实现对照通过。边界改动为通用单位名、借用现有曲线表、导入和跨 crate 可见性。插件新仓库只保留通用代码和数值夹具。见 [algorithm-identity.json](algorithm-identity.json)。

独立仓库 `main` 的提交 `3277c9db62daadf687e2d96d1840cd0d6a685c92` 已与远端核对。Rust 49 项单元测试和 1 项独立 Bevy 插件测试通过。撤去插件注册时，独立用例在注册表缺失 `RigDefinition` 处失败。按字节恢复后在同一 target 重编并通过。格式和 rustdoc 验证通过。Clippy 原有告警为高精度常量、多参数、条件块及两种测试写法，排除这五类后以 `-D warnings` 通过。

初次独立锁解析遇到已撤回的 `async-executor` 1.14.0。沿用原锁已解析的版本后重新生成独立锁文件。迁移的反射往返测试同时需要原 `ron` 开发依赖。原生报告无损压缩，过程日志已移除本机路径。
