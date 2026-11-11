# Playground 功能回归

公共 runner 的场景、退出码、功能校验和渲染结果保存在各目录的 `runner.json`。`process.report.json` 记录 watchdog，`report.json.gz` 保留完整场景报告。对应的 `report.json.archive.json` 记录原始字节数和 SHA-256，压缩后已解压核对。

在仓库根目录可还原报告并复验。下面使用台阶场景作为示例，其他场景替换目录名即可。

```sh
gzip -dk docs/audit/bf144/ragdoll-avian-regression/stairs/report.json.gz
shasum -a 256 docs/audit/bf144/ragdoll-avian-regression/stairs/report.json
python3 tools/validate_animation_visual_evidence.py --report docs/audit/bf144/ragdoll-avian-regression/stairs/report.json --check --functional-only --min-screenshots 0
```
