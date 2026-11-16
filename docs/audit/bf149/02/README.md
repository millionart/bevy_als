# BF149.02 源单位归属和 Git 依赖

源程序与 .01 新读的 UE 资产使用同一组 Construction 和 Forwards Solve 入口及 45 条单位调用。`/Script/ALS` 的八种运行时自定义单位保留源标识与实现，位于 `src/control_rig/`。原 `FAlsRigVMFunction_Clamp01Float` 转换记录继续由编译器提供，生成程序以普通数学指令执行。插件处理原生 FK、IK、层级、曲线、弹簧和生命周期单位。已比对本地 UE 声明、源工程实现及迁移两端的 Rust 单位分发。 [源单位](source-unit-owners.json) [调用对照](unit-dispatch.json)

图节点传递 Pose 和曲线，按 Construction、Forwards Solve 和写回顺序执行。角色程序夹具位于动画图扩展，Avian 查询通过 trace 回调传递，运行时资产哈希一致。两个 Rig 作者性合同指向 Git 管理的新读证据，已通过源证据和图节点、连线、位置、编辑器以及运行时资产边界检查。

Git 依赖锁定 `ccca1b0dc7e855a63c6654430b41d3bedc214d3a`，插件的原生单位和 39 项独立测试已推送。试用全局 Bevy 序列化特性会扩大依赖配置，最终提交只为 `bevy_math` 启用序列化。锁定后已有 562 个包的特性集合逐一一致。应用、插件和固定 revision `6d706f61` 的 Bevy Animation Graph 使用同一个 Bevy 0.20.1。 [依赖身份](dependency-identity.json)

消融仅移除 `FAlsRigVMFunction_IsGameWorld::Execute` 的应用分发分支。原程序的 Forwards Solve 在指令 2 报告该源单位签名不可用。按字节恢复后在同一 `target_game` 重编，保留了原始失败及恢复报告。

恢复后，2119 项单元测试和 37 项集成测试通过。`target_game/play/examples/manual_playground` 的同一份构建通过正式 `startup`、`control_rig`、`foot_ik` 功能验收。各场景收集到 18 根骨骼，图错误为 0，Foot IK 覆盖 12 个动作窗。性能超预算项保留在报告中。 [构建身份](binary-identity.json) [运行验收](runtime.json)

解压各场景的 `report.json.gz` 后，可用 `tools/validate_animation_visual_evidence.py --report <文件> --min-screenshots 0 --check --functional-only` 复核。原始报告长度和 SHA-256 见 `runtime.json`。过程日志已移除本机路径。
