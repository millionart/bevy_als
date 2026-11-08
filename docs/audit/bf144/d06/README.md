# BF144-D06 Playground 世界帧时钟

fresh `L_Als_Playground` 的 WorldSettings 原生读取给出最小/最大未缩放帧长 `0.0005000000237487257` / `0.4000000059604645` 秒，三个世界时间倍率均为 1。本地 UE 先应用有效倍率，再将帧长限制到同倍率缩放的区间；该限制不同于物理模拟上限。原始世界读取及压缩哈希、源代码和 Bevy 0.19/0.20 时间实现摘录在本目录。

`tools/import/import_world_time.py` 从原生 WorldSettings 生成单独的 target 配置，不修改导出的原始世界资产。Playground 接入 `SourceWorldTimePlugin`：保留原始 `Time<Real>`，从上一虚拟时钟状态以受限 raw delta 重算 `Time<Virtual>` 和当前 `Time`，保留暂停、速度和累计时间；限制适用于缩放前的世界帧长，并为虚拟时钟发布对应倍率的最大值。实现使用公开 Bevy 时间接口，同时覆盖 0.19 的先限 raw 和 0.20 的先限缩放值两种行为；原 `SourcePhysicsTimingPlugin` 的 1/30 秒物理限制保持独立。时长以纳秒存储，与 UE float 之间仍有表示精度差。

公共视觉 runner 和 validator 现允许原始 0.3/0.5 秒输入，并依据生成配置检查实际受限帧长。已写世界时钟倍率/暂停/累计、真实 CMC 与独立物理体、导入器及 validator 自测代码；全部尚未运行。配置可复跑：

```sh
python3 tools/import/import_world_time.py --source docs/audit/bf144/d06/world.native.json.gz --output assets/ALSExtras/Levels/L_Als_Playground/world_time.runtime.json
```

测试、消融、完整图审计和真实 Playground Gate 仍待所有静态修复及 Bevy/依赖升级后执行；旧 `target/bf144/03/world_clock/` 原始报告缺失。
