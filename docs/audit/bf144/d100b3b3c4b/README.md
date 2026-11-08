# BF144-D100b3b3c4b：源凸包接触生成组合

源在无法复用 manifold 时保留 GJK simplex，再执行 GJK、带支持偏移的剔除、选面分类和面或边边点生成；结果及 simplex 分别供后续求解与下一次重建使用。此处针对已证实的动态 box 对静态凸包，后者有效 margin 为零。`source-facts.json` 保存生命周期和组合源码。

fresh Bench 连续帧的平移和旋转两组均未复用原 manifold，下一帧分别生成四点；请求、原始读数、摘要、身份和哈希见 `native-identities.json`。精简样本可运行 `python3 tools/import/export_contact_restore_probe.py --source docs/audit/bf144/d100b3b3c4b/warm.native.json.gz --output docs/audit/bf144/d100b3b3c4b/warm.native-samples.json` 重建。另复用 c2a 的面/边样本和 c4a 的剔除边界。

已组合源 GJK、分类、裁剪与边边投影，并在无接触时保留求得的 simplex；已写面、边、剔除以及两组热启动重建点序控制。旧 reader 未输出直接冷/热 GJK；统一 Gate 重建后须重跑请求并执行 `tools/tests/test_physics_convex_manifold_probe.py`。真实接触生命周期、测试、消融、全量转换 diff 与 Playground 待后续。
