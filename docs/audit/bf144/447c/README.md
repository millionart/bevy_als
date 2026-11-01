# BF144.447c Head共同输入核与首Look差异

复用已封存原Playground `.447b` 的实际319前后参数、Look pin及 `.446f` 同engine帧source318。当前公开 `update_head_state` 从默认状态连续重放目标319输出，全部逐值相等；再分别使用各帧源实际前态与typed输入执行相同公开核，318输出也全部逐值相等。这是逐帧共同输入控制，没有宣称源连续状态由target历史自产。

source78/target79首次Look差异，仅替换 View yaw：目标44.999996185302734度→源45度，即使保留目标前态也恢复源Head三个yaw字段；只替换源前态仍得到目标输出。实际Look消费pin等于目标YawAmount。已排除当前首差来自Head spring算序，不需要新增原生数学探针，不据此新增编号。上游View producer的角度表示、body/view来源仍须沿源合同独立审计，不能按名称归入D237或D258。

原生输入范围由CDO ThirdPerson/ViewDirection/非瞄准、现有reader无模式命令及全部318帧无相对基座旋转约束。非第一人称路径不消费unscaled delta。Head half-life来自 `.447` 原生设置，当前目标默认值逐值一致。

删除唯一native View yaw替换因素后，最小首Look断言以101退出；恢复fixture源码原始字节，在同一target的六个冻结库上重编，完整报告byte精确恢复。396现行身份、六库与原main未变。仅新增Git审计证据，无生产、资产或转换器修改；既有 `.447b` 真入口功能Gate保持，未重复运行同一场景。

前轮 `restore-final` 后缀误走旧脚本删除分支而失败，且覆盖了当轮备份；该轮不能作为恢复证明。修正为先解析mode并在备份前验证替换点后，重新建立 `verified-baseline`，完成 `ablated-verified` 与 `restored-verified`。最终Gate只引用这三个独立报告。

复验：`pwsh docs/audit/bf144/447c/run_replay.ps1 -Label new-check -Build`。结果写新label；封存结果不覆盖。截图0、性能暂停。完整Root、其它输入历史与custom attributes仍未闭合。
