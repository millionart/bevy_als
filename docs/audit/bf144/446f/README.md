# BF144.446f 匹配初始化与普通tick的engine帧

可选boolean `advance_before_first_world_tick`默认False。仅在原初始化快照完成后、首普通World.Tick前将GFrameCounter加1；不增加World.Tick、不修改delta、world seconds、actor或动画状态。默认同engine帧合同保持。

目标.e实际init App5/首普通App6。源原默认init/首world同engine100，NativeThreadSafeUpdateAnimation每engine帧只执行一次，首world保留PoseState0。控制仅将首world放到101，native回调执行并读已有map.PoseStanding1，Default_Poses节点首步即更新，全部318步的该节点资格与目标相同。完整init内容、world steps/time/delta以及外部输入保持；因此原启动曲线“发布丢失”候选被此反例否定，不新增编号，不修改target projection。

消融移除首帧推进使同一请求完整回到旧同帧character probe，资格Gate失败；typed guard移除使字符串被接受。源码原始字节/SHA恢复、同native package重编后受控完整318+2报告精确恢复，默认无新flag完整旧318+2也精确兼容。基线/消融/恢复各最终包源码SHA在运行前核验。

此项只关闭该启动候选及对应帧资格，不归并所有后续clock/physics余差或声称Root整体等价。冷启动竖直力系数、完整Root/非零view/custom attributes仍继续审计。截图0、性能暂停。原始报告无损gzip、压缩哈希、请求、源事实、消融/恢复和身份迁移全部Git管理。

复验为 `pwsh docs/audit/bf144/446f/run_check.ps1 -Label restored`、`pwsh docs/audit/bf144/446f/run_python.ps1 -Name analyze -Case restored`。新native运行另用新Label，不能覆盖封存数据。
