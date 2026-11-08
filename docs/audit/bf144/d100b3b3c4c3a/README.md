# BF144-D100b3b3c4c3a：源凸包候选 bounds

源 box/convex 在非 MACD 路径先比较带 cull 的世界 AABB；上帧未激活时，还把另一侧 bounds 分别变换到两侧局部空间进行双向检查。上帧已激活只跳过局部检查，世界 AABB 仍必须相交。当前 PA 的 MACD 已由 BF144 原生记录确认为关闭。源条件、浮点单位和 AABB 变换见 `source-facts.json`。

已按源 double 厘米、非均匀及负缩放实现候选判断；编写源 Bench 的 2.4 cm 正间隙 hull、相切边界、旋转细长形体和上帧激活控制。原生聚合及资产身份由 `identities.json` 引用；源候选判断本身尚未连接真实 solver。

运行 `python3 tools/import/import_contact_bounds_settings.py --facts docs/audit/bf144/d100b3b3c4c3a/source-facts.json --native docs/audit/bf144/d100b3b3c4c1/aggregate.native.json.gz --output src/als/settings/contact_bounds_checks.rs` 可生成目标开关。旧 reader 未记录 bounds CVar，目前使用源默认值；已补采集，重建后须重采并执行原生候选探针。缺组接入、测试、消融、全量转换 diff 和 Playground 待后续。
