# BF144.445a 原生 asset tick 与时钟只读字段

现有 `CharacterAnimationProbe` 增加可选 boolean `capture_asset_player_ticks`。未指定时不生成新字段，旧报告合同保持。只读取所有主/linked 实例 proxy 的同步组与 ungrouped read buffer，通过 `FAnimTickRecord.DeltaTimeRecord` 与节点公开 getter 的同一地址匹配；不输出地址，不干预 update/evaluation。

字段 `sync_read_buffer_tick_present` 表示当前已完成同步批次有该节点的记录；与现有实例 update/evaluation counter、before/after-world 阶段联合使用，不能脱离批次声称每次观察都重新 tick。`tick_accumulator`/`tick_previous` 对 BlendSpace 是 normalized；当前 `GetCurrentAssetTime()` 返回最高权重样本的秒数，`blend_samples` 同时保留实际秒与缓存秒。Sequence 记录的时间为秒。没有关联记录的声明槽不补造零时钟。

本 source240 的20,400声明行中，实际关联1,560行；544个 BlendSpace current time 逐值对应最高权重样本缓存秒。1,578行缓存权重为正但本批次无 tick，证明不能将该缓存单独作资格 oracle。这是读取能力与观测口径，不登记为新的移植缺陷。

消融取消指针匹配、typed flag 和两个可选输出 guard，分别使实际 tick 全丢、字符串请求被接受及无 flag 旧报告被污染。恢复采用 Git 目录内压缩源码原始字节，并在同一 native package 重编。baseline/restored 的完整 character probe 和原无 flag240必须逐值一致；外围整体资产/图加载元数据不作为字节等价结论。构建/运行均用现有 watchdog；截图0、性能暂停。

所有原始报告无损 gzip、请求、源事实、消融/恢复结果、工具源码恢复字节和身份迁移均在本 Git 目录。压缩报告的字节数与解压 SHA 在 `compression.json`；旧 source240保持 `../443/source.native.json.gz`，不重写。读数复验用 `pwsh docs/audit/bf144/445a/run_check.ps1 -Label restored`。新运行使用新 Label，不覆盖封存文件。
