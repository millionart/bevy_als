# D100b4b3a 源网格邻接与接触特征

源仅为已采集三角建立局部邻接，按采集次序登记每个排序顶点对。单边保留第一个三角，有更多入边时第二槽保留最后一个三角。接触以第二支持点求重心权重，先检查权重接近 1 的顶点，再检查接近 0 的相对边。非退化、退化线和退化点各沿源分支，顶点 ID 始终来自 cooked 拓扑。网格归并与箱体 SAT 共用源三角法线，胶囊逐面保留它的独立退化门槛。

`tools/import/import_mesh_contact_settings.py` 从本地源码与 D100b 原生记录生成 `src/als/settings/mesh_contact.rs`。原生两项排序和强制 MACD 关闭，全部 21 个 PA body 的 `bUseMACD` 为 False，法线修正和边界面替换启用。源 midphase 在普通路径保持零相对移动，故沿此配置消费网格生成。D104b3b 记录的 Stairs02 面 252 和 270 共享 cooked 顶点 75/76，面 252 的两个原生点位于该边。已写阶梯、退化与多邻接的 focused 用例。编译、测试、消融和 Playground 待统一 Gate。
