# BF110 结构化图离线展开

展开器只处理作者资产，不参与运行时。它从结构模板中读取节点、未托管 edge、`io_spec` 与 editor metadata，再从 context 合同生成 pose、独立 time 和自动发现的曲线 edge。合同放在 `tools/animgraph_contexts/`，避免 AssetServer 把它作为未知运行时资产加载。

模板不得包含已展开的 context edge。展开器会拒绝这种输入，避免把生成 RON 误当作作者源；对同一模板重复展开必须得到相同结果。

当前根图的 348 条 edge 中，230 条由 11 个 context 管理，118 条保留为独立 edge。独立 edge 包含控制输入、专用 time 分支、常量和非 context 数据，不会因可读性优化而被合并。

标准流程：

```text
python tools/expand_animgraph_context.py --manifest tools/animgraph_contexts/ab_als.animcontext.json --source assets/graphs/ab_als/ab_als.animgraph.ron --template assets/graph_sources/ab_als.animgraph.structure.ron --make-template
python tools/expand_animgraph_context.py --manifest tools/animgraph_contexts/ab_als.animcontext.json --template assets/graph_sources/ab_als.animgraph.structure.ron --output assets/graphs/ab_als/ab_als.animgraph.ron --baseline docs/bf110/ab_als.context-baseline.json --expand
python tools/expand_animgraph_context.py --manifest tools/animgraph_contexts/ab_als.animcontext.json --template assets/graph_sources/ab_als.animgraph.structure.ron --output assets/graphs/ab_als/ab_als.animgraph.ron --baseline docs/bf110/ab_als.context-baseline.json --expand --check-output
```

审计同时比较非 edge 图内容与有序 edge 列表，因此节点、position、IO、默认值、metadata、edge 数量与连接都会被保留。未来新增 source output 和匹配 target prefix input 后，展开器会自动加入新的 curve edge；未声明的 input 不会被猜测或传播。
