# BF144-D104b2a：源凸包边界与身份

fresh 原生读取 15 个环境凸包，源 bounds 与已有离线拓扑顶点逐轴最小/最大值完全一致。另读取三个非均匀缩放样本（含负缩放）；按源缩放两端点并取最大轴长度，结果以源 float 厘米比较完全相同。fresh 15-shape 原始字节与 D100b3a 证据相同，因此共享其受跟踪压缩文件；请求、压缩原始读数、watchdog、源资产身份及哈希见 `native-identities.json`，18 个精简样本见 `bounds.native-samples.json`。

无需新建重复运行时资产：`SourceCompoundOrder` 携带原资产名，按原始 hull 索引从既有 topology 顶点取得对应边界。Playground 将真实资产名传给标记；新增 18 个原生样本、无效身份及现有 Arch 碰撞构造测试。运行时实际复用消费留待 D104b2b；构建、测试、消融和 Playground 验证待统一 Gate。
