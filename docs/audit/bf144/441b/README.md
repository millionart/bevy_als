本目录是 BF144.441b 的长期证据，随 Git 提交。`summary.json` 定义通过范围，`evidence-manifest.json` 绑定文件字节。此前构建目录中的 12018 身份链、原 target240 和冻结库已丢失，本目录不声称恢复了它们。

`character.native.json.gz` 是完整新原生报告的无损 gzip，原字节 SHA 与压缩 SHA 见 `storage-manifest.json`。`source-parameters.json` 保留新的 240 帧参数事实，`scalar-check.json` 区分原切线拟合与单位求值；没有截图、性能或完整角色等价结论。

从项目根目录运行 `pwsh -NoProfile -File docs/audit/bf144/441b/run_script.ps1 -Name check_source` 可复核原生读数与模型；`check_scalars`、`check_controls` 对应标量和消融恢复。`prepare_excerpt` 提取当前目标方法原文，`run_excerpt.ps1 -Label <name> -Build` / `-Label <name>` 编译与执行摘录；可执行文件临时放 tools/tmp，长期数值结果保存在本目录。

重新提取源数据使用仓库 `tools/run_ue_source_reader.ps1`、本目录请求及本地配置，输出到新的证据目录，保留本次原始报告。身份校验需要通过 `ALS_UE_SOURCE_ROOT` 注入本地 UE 源码根目录，再执行 `check_identity`。
