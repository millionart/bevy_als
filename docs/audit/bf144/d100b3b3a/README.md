# BF144-D100b3b3a 支持顶点邻接面选择

源 `SelectContactPlane` 在支持顶点邻接面不超过三个时按缓存顺序检查；超过三个时按全部面次序检查。候选面必须处于搜索距离内且法向与查询方向足够相反，平局保留先前候选；无合格面时 box 按源 `MaxAxis` 平局规则回退，convex 按最反向平面回退。缩放凸包的候选计算使用源单精度及逆缩放法向，未缩放路径使用双精度厘米。

`source-facts.json` 保留源选面、缩放和回退代码；`native-face-evidence.json` 与 `../d143/bench-foot.native-sample.json` 保留 Bench 原生 simplex 对照。该 simplex 的凸包支持点与源顶点 10 完全相同，但读取器尚未直接导出 GJK 末次支持顶点，不能把它们视为同一事实。

执行 `python3 tools/import/import_box_contact_adjacency.py --facts docs/audit/bf144/d100b3b2/source-facts.json --output src/avian_ext/source_box_vertex_planes.rs` 与 `python3 tools/import/import_contact_face_search.py --facts docs/audit/bf144/d100b3b3a/source-facts.json --native docs/audit/bf144/d143/gjk.native.json.gz --output src/als/settings/contact_face_search.rs` 可重建目标数据。旧 reader 未输出搜索距离 CVar，本轮使用源码默认 1 厘米，重建后必须重采并对比。已编写源 simplex、邻接平局、负缩放与 box 回退测试；实际 GJK 顶点身份、运行时接入、测试和消融仍待后续条目与统一 Gate。
