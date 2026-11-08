# BF144-D143 源核心碰撞边距

fresh PhysicsSettings 与 PA GJK 探针保留在本目录；本次世界设置与 `../d81/settings.native.json` 字节相同，故共用该原始记录并保留本次 reader 报告、请求和哈希。`gjk.native.json.gz` 是本次完整原始结果，`source-facts.json` 保留设置覆盖、box 核心支持点及动态/静态多面体有效 margin 的源码。

世界比例为 0.05、上限为 10 厘米；当前 hand 和 foot 原生有效 box margin 分别约为 0.35、0.6 厘米，静态环境侧均为零。源默认 `MarginFraction`/`MarginMax` 覆盖为 −1，`ConvexZeroMargin` 默认 0；旧版 reader 没有记录这三个运行时 CVar，因此本轮生成使用源码默认，并用已有手脚原生读数核对，不能据此推断未观察的小形体边界。reader 现已增加这三个字段及检查，统一 Gate 重建后必须重采、重生成并对比目标数据。

执行 `python3 tools/import/import_convex_margin_settings.py --facts docs/audit/bf144/d143/source-facts.json --settings docs/audit/bf144/d81/settings.native.json --native docs/audit/bf144/d100a/contact.native.json.gz docs/audit/bf144/d143/gjk.native.json.gz --output src/als/settings/convex_margin.rs` 可重建目标设置。已写箱体大小、上限及动态/静态组合测试；核心查询与正常求解器消费属于 D143b，测试、消融和真实 Playground 待统一 Gate。
