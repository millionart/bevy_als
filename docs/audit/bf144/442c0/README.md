# BF144.442c0：实际 player viewpoint 只读快照

为 cold fallback 审计补唯一缺失观测：`capture_player_view: true` 时直接读取实际 controller 的 `GetPlayerViewPoint` 返回值、camera cache time/rotation、viewtarget 所属和 actor 位置/旋转。它不设置 viewtarget、camera manager、cache、输入事件或相机更新阶段；无 flag 的请求不多做查询或增加字段。生产 ALS、转换器、图、资产和六个运行库不改。

旧 reader 在同一请求下缺 `player_view`，先稳定失败。新 native 三帧/七快照全部 cache time 0；控制角 60°时返回 viewtarget actor 的 0°，位置为 actor 的 `(0,0,92.15)` cm。该 commandlet world 的 tick 没有更新相机缓存，故这不是正常 local-player 相机循环，也没有 native warm-cache 动态结论。检测器对实际存在的 cache 状态验证返回；合成 self-test 覆盖 warm rotation 拒错。

消融同包重编分别删掉 getter、boolean 类型检查及默认关闭条件：返回零位置被拒；字符串 flag 从 native 7 变 0；无 flag 的旧请求出现新增字段。三项同一编译变体用彼此无关的输入条件区分，结果见 `ablation-check.json`。`ablate.py restore` 按替换逆操作恢复源码，SHA 必须等于 `reader-final-sha256.json`，同包重建后才可认定恢复。

`check.py` 对比整个 `character_animation_probe`，七快照的原字段全部不变；恢复输出完整 character probe 与 baseline 精确相同；旧 source 240 帧整个 character probe 不变。顶层 `asset_objects` / `graphs` 含另次进程生成的 GUID，最初整包比较有 176 处差异，完整保留于 `before-baseline-differences.json`；本项不宣称外围图 GUID 相同，也不据此复开图。

首版构建因 controller 隐藏 actor rotation 失败，删除重复观测；首轮 native 推翻了会自动更新 camera cache 的假设；wrapper 起初将 native 7 归成 shell 1，现从 watchdog 的完成结果读取 native exit code。失败证据保留，不计为成功恢复。

复现于本提交检出树中，配置继续由忽略的 `tools/env.local.json` 和 `ALS_UE_SOURCE_ROOT` 注入：

```sh
pwsh -NoProfile -File docs/audit/bf144/442c0/build.ps1 -Label replay-build
pwsh -NoProfile -File docs/audit/bf144/442c0/run.ps1 -Label replay-native
python3 tools/tests/test_character_player_view_probe.py --self-test
python3 tools/tests/test_character_player_view_probe.py --input docs/audit/bf144/442c0/restored.native.json.gz
pwsh -NoProfile -File docs/audit/bf144/442c0/run_python.ps1 -Name check
```

reader/build/Python 均复用已有固定无进展上限与机器报告。原始 native JSON 无损 gzip，raw/gzip SHA、字节长度见 `compression.json`；需长期保留的输入、原始输出、失败、合同、身份迁移与摘要全部 Git 跟踪。构建产物和临时日志仅作可重建缓存，不作长期证据。截图 0，性能验收暂停。
