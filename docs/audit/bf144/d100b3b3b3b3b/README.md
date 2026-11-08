# BF144-D100b3b3b3b3b 源 GJK 与 EPA 状态衔接

源 GJK 核心距离小于 epsilon 时把 simplex 和支持函数交给 EPA。EPA 成功或达到迭代上限时，源使用 EPA 法向、穿透及两个见证点，并加回有效 margin；初始 simplex 无效时使用源接触法向和 EPA 给出的距离，EPA 扩展退化时保留上一有效 GJK 法向。每种情况都保留最后一次支持查询的顶点索引，GJK simplex 缓存保持源保存时的状态。

`source-facts.json` 保存源状态映射，fresh 冷/热原始报告及身份分别见 `../d100b3b3b1/` 和 `../d100b3b3b3a/`。已编写坏初始 simplex、EPA 上限及退化状态控制，冷/热 Rust 测试也支持通过 `ALS_SOURCE_COLD_GJK_REPORT` 与 `ALS_SOURCE_WARM_GJK_REPORT` 输入重建 reader 后的新原生报告，逐项核对包括重叠 EPA 在内的法向、见证点、穿透与末次索引。

当前源 GJK/EPA 仍是独立合同，尚未接入碰撞求解。原生直接末次索引、测试、消融和 Playground 待统一 Gate。
