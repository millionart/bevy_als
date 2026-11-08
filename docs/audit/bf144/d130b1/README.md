# BF144-D130b1：胶囊/凸包选面与分支

fresh 源实现使用同空间 GJK 的凸包末次支持顶点查询邻接面；先按源搜索距离与法向选择面，再以 `abs(axis·normal) < 0.01` 判断圆柱，并以源 plane 阈值及 `abs(axis·plane_normal) < 0.707` 判断端帽辅助点。非均匀和负 scale 按现有源凸包选面合同处理。源代码摘录与哈希见 `source-facts.json`，head/Arch 两个原生分支及 GJK 证据引用见 `native-identities.json`。

已复用源邻接面搜索，增加胶囊分支分类及原生/严格边界测试。`tools/import/import_capsule_convex_settings.py` 从源提取 double 阈值、两处一致的裁线容差及 float 接触距离比例，生成 `src/als/settings/capsule_convex_contact.rs`；比例 CVar 的零和负值按源平方使用。旧 reader 不记录该 CVar，当前采用源码默认值；已扩展 reader，重建后须重采并重生成配置。

生成命令：`python3 tools/import/import_capsule_convex_settings.py --facts docs/audit/bf144/d130b1/source-facts.json --native docs/audit/bf144/d100b3b3e/endcap.native.json.gz --output src/als/settings/capsule_convex_contact.rs`。点生成、reader 重建、全量转换 diff、编译、测试、消融与 Playground 均待后续统一 Gate。
