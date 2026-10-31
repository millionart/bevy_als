本目录是 BF144.442b0 的长期 Git 证据，仅补原生输入数学读取能力，未宣称source/target输入等价。当前reader宿主无ALS模块依赖，因此按已读 UAlsVector 内联体及 Input_OnMove 的操作顺序调用 Core FMath/FVector API，没有复制源头文件或新增依赖。

`request.json` 覆盖轴向、斜向、过幅、零输入、角度及模拟量。`before.native.json` 缺新增合同，schema检查必须失败；`initial.native.json` 为完整float角度/弧度、2D clamp、世界方向和CMC clamp阶段。非法yaw/axes类型及维数各退出7。

去掉clamp时上游clamped轴/世界输入不符，即使后续CMC clamp把最终方向收回；去掉yaw数字类型检查后，JSON取数接口接受数字字符串。源码bytes恢复同包重建后七样本逐字段恢复；旧720范围映射请求的完整geometry probe保持精确。失败的常量false消融编译报告保留，随后改为实际删除计算。

`reader-migration.json` 明确绑定reader源码、包源码、包动态库和工具说明四身份变化；生产ALS/Rust/引擎/转换器/源资产及六份实际库保持不变。复现使用仓库reader、目录请求和本地配置；完整新运行请输出到新的Git证据目录，禁止覆盖本次原始报告。
