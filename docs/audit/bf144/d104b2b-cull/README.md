# BF144-D104b2b：复用后重新判定 cull

fresh Barrel/foot 两帧原生样本均从最小 phi 约 2.9 cm 开始，源 `TryRestoreManifold` 均成功并保留 4 点。向外移动 2 mm 后最小 phi 约 3.1 cm，向内移动 1 mm 后约 2.8 cm。源 `GetPhi()` 取最近有效点，复用后仍用 `GetPhi() <= CullDistance` 决定是否激活；因此 3 cm cull 下前者不激活、后者激活。这一激活结论由原生点读数与重新读取的源分支共同推导，不是单独的 midphase 原生输出。

共享逐 hull 生成现用复用结果中的源精度有效点重新判定 cull；越界时不输出接触组或活动点缓存，仍保留已有 simplex。已写两帧原生跨界及内侧控制；测试因用户指定的统一验证顺序尚未运行。请求、原始报告、watchdog 结果和哈希见 `native-identities.json`，源片段见 `source-facts.json`；精简样本可用 `tools/import/export_contact_restore_probe.py` 从压缩报告重建。
