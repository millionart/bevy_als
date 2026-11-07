# BF144-D267 地面动力曲线的原生键

本轮原生读取三个 MovementSettings 的实际曲线引用，并经仓库 reader 重读 Normal、Responsive、Sluggish 共九轴的键、插值和外推；请求、原始读数、watchdog、源资产身份及 UE 曲线源码见本目录。生成器只接受源设置确实引用的曲线、未加权线性/三次段与恒值外推；线性段转换为等价 Hermite 切线，三次段使用原生切线，长度值和切线离线投影为米制。

独立 `grounded_dynamics.runtime.json` 替代原硬编码拟合数据，保持 `GroundedDynamicsCurves` 的 SI 接口。与 `.513` 完整三profile/九轴旧数据逐字段对比仅7个切线字段改变：已知 Normal/Responsive 各1个，fresh Sluggish 另5个；键时间、值和其他字段保持。Sluggish 是同一拟合根因的源数据修复，不据此声明默认场景消费或完整运动恢复。

本项原SI F32生成器及报告保留为历史证据，需在对应提交复验；后续 D268c 为保持厘米计算精度将目标键拓宽为SI F64，当前重生成入口见 `docs/audit/bf144/d268c/generate.ps1`。已写两处已知原生切线、线性段和生成器完整 diff/坏权重用例。Hermite 算序仍归 D206。全部静态修复及 Bevy/依赖迁移后执行 focused 测试、消融、全量 AnimBP/递归依赖转换及完整新旧 diff、图静态审计和公共视觉 Gate；恢复旧拟合值应使相应用例失败，源码字节恢复并同 target 重编后复验。上述测试和消融尚未运行。
