# 证据与测试夹具

[移植差异](../parity-report.md)链接当前验收和主要源证据。仓库保留 Rust 和工具直接读取的夹具、相关原生读数与复核边界。夹具保留原路径和字节。[清理清单](bf154/cleanup/retention.json)列出扫描保留的输入。

已完成的 [BF144–BF153 清单](../archive/2026-11-18/BF144-BF153.md)对应 Git 标签 `als-port-acceptance-2026-11-18`。该树保留移除前的文件、脚本和报告的原路径。历史记录里的 `target/` 或 `tools/tmp/` 引用当时的本地输出，是否已入 Git 以对应条目为准。

```sh
git ls-tree -r --name-only als-port-acceptance-2026-11-18 -- docs/audit
git show als-port-acceptance-2026-11-18:docs/audit/bf150/scene-regressions/README.md
```
