# bevy_als 维护与验收

项目以 ALS-Refactored v4.18 与 UE 5.8 为对标基线。[差异报告](docs/parity-report.md)记录范围和已接受差异，[移植验收](docs/audit/bf154/baseline/README.md)保留整理前的基线。

后续工作从 [CHECKLIST.md](CHECKLIST.md) 的首个未完成条目开始。按 [agents.md](agents.md) 核对依赖、取证和验收，再逐项勾选并提交。源码和资产按[源归属](src/README.md)维护。重新导出资产之前，先在当前清单登记变更范围。

常规验收使用工作区测试，并按改动选择[公共 Playground 场景](tools/examples/README.md)。

```sh
cargo test --workspace --locked --jobs 2 --features playground -- --test-threads=1
```

插件入口、宿主资产根或依赖安装变更还需通过[独立项目夹具](docs/audit/bf154/consumer/README.md)。报告与可复验夹具按[证据规则](docs/audit/README.md)维护。已关闭的批次见[归档索引](docs/archive/archivedchecklist.md)。
