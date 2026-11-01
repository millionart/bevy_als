# BF144.452a：当前完整 Head 旋转阶段

复用 `.451a` 原 Playground 的真实输入、有效 alpha=1、82 路径骨架，以及 `.451b` 六批 native current/current 输出；不新增 runtime，不更改生产代码、资产、reader 或引擎。`run_replay.ps1 -Label <label> -Build` 在相同 `target_game/play/deps` 六库上编译静态探针，`run_python.ps1 -Name analyze -Case <label>` 校验 318 帧全部骨骼和八种阶段组合。运行前将 `ALS_UE_SOURCE_ROOT` 指向本地 UE 源码后，可重新执行 `facts` 与 `identity`。

`110` 表示加性输入和组件乘积均归一化、局部空间一次归一化，完整精确重放当前公开核的 25,440 个旋转行；`001` 表示全权重源阶段顺序，但仍使用目标 f32/glam 数学。八域分别控制 delta-N、product-N 和第二次 local-N，只改变旋转。当前域单独去 delta-N 改变 641 骨行，去 product-N 改变 18,834 行，增加第二次 local-N 改变 3,475 行；因素相互作用，不能相加。

全部八种组合仍与 native 留有 23,027–23,041 个旋转骨行差异。阶段模型证明必要因素，不证明全部数学恢复。后续须独立校准相同输入的源 F64/ARM64 算术；本项不新增差异编号，不把全部余差归 D241 或精度问题。

`controls.ps1` 分别物理删除三个因素分支，对应分析须因缺少因素信号失败。源码以字节恢复后，在同一 target 重新编译，完整输出、分析与逐帧表均与原字节相等。`negative-controls.json` 长期保存最小失败，临时日志不是唯一证据。大输出无损 gzip，原始长度与 SHA 见 `compression-map.json`；分析、finalize 支持压缩输入。重新执行 controls 前须移走既有保存文件，防止覆盖原消融基线。

证据与外部输入清单由 manifest/summary 绑定 Git 跟踪与 SHA；构建二进制位于临时目录，仅为可重建产物。截图 0，性能暂停，GPU 周边不验收；完整 Root/ALS 等价仍未完成。
