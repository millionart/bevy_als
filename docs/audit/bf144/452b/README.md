# BF144.452b：源 Head 旋转算术与设计消融

以 `.451b` 六个原始请求和 native 输出的 1,272 组真实四域输入、79 骨父链为唯一数据，独立计算 local→mesh、全权重加性、mesh→local、API 与 node 两次归一化。全部 100,488 个旋转骨行与 native F64 逐值精确，CC/SS 各 318 组分别连接真实 target/source 输入。只验旋转；不宣称完整 Root、Rig、属性或其余通道等价。

先通过 `prepare` 重建 `inputs.json`，再运行 `run_replay.ps1 -Label <label> -Build` 与 `run_python.ps1 -Name analyze -Case <label>`。设置 `ALS_UE_SOURCE_ROOT` 后，`facts`/`identity` 复核本地 UE 方法与当前396文件/六库身份。二进制临时保存，可从 Git 中的源、请求和锁定库重新构建。进展 watchdog 复用项目现有已验证入口。

同一 current/current 输入，固定源 F64 算法后，每个因素单独改变的最终 F32 旋转骨行如下；不能相加。

| 单因素 | 改变骨行 |
|---|---:|
| 提前归一化加性 delta | 583 |
| 提前归一化组件 product | 594 |
| 只保留一次 local normalize | 57 |
| 相同 dot 改标准 sqrt/倒数 | 1 |
| 每个源 primitive 结果投 F32 | 22519 |

最后一项保留源 F64 primitive 后再投影，不能称为当前 glam F32 运算。普通 sqrt 的唯一 CC 投影反例为 frame306/foot_r 的 x：源投影 −0.012283205054700375，控制 −0.01228320598602295。五因素在全部四域均有 double 和最终 F32 信号；所有域完整统计及首反例在 analysis 中。没有将每个 ULP 另报新差异。

设计消融发现，手写 F64 QuaternionMultiply2 换为已安装 `DQuat` 后，全部 7,632 份因素输出字节不变，故删除手写乘法；旧源码和删减证明留在 `replay.before-ablation.rs` / `product-ablation.json`。最初期待该替换失败的实验被成功结果否定，随后改用简化源码重新执行全部五因素、native 校准和字节恢复。这个结论只覆盖现行矩阵，新增输入仍须过原生精确合同。

NEON normalizer 使用 fresh source Dot4/FMA、estimate 和两轮 refinement。物理换标准 sqrt 后首 `000:cc` root 的 w 为1，native 为0.9999999999999999，精确合同失败。五因素物理删除各自被缺信号合同拒绝。每轮源码字节恢复后同 target 重编，baseline/restored/primitive-restored 的完整输出、分析和表格字节精确。最小失败在 Git 管理的两个 negative JSON 中，临时 stderr 不是唯一证据。

12 个大输出无损 gzip，解压字节长度/SHA 可核；输入 JSON 保持可直接重跑。manifest/summary 绑定本项与外部依赖的 Git 跟踪、长度和 SHA，未将长期证据放入 target。截图0、性能暂停、GPU周边排除；原生产/reader/引擎/原资产不改，272 项既有差异未修。
