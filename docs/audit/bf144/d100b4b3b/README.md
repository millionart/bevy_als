# D100b4b3b 源网格接触所有权

源将接触法线与三角法线的点积转为 float 后判断面接触。共享边已有面接触时沿用原三角。其余情况比较已有最大点积，换主时禁用已保存的两个槽并追加新点。面边点同时给当前三角和邻三角增加计数。顶点只留一个槽，已有面点优先，较高点积可原位替换。输出保留三角局部索引、原 cooked 面号、点类型和有效标志，供后续两轮访问及法线修正消费。

源 `Contacts.Reset(min(overlapCount * 4, 1000))` 的真实 `Max()` 经过分配器量化。每个实际采集三角最多四点，因此采集三角数乘四可作为小集合上界，实际量化容量决定大集合的停止位置。登记器显式接收源容量，满时在识别特征前退出。reader 已准备 `capacity.request.json` 直接读取 `TArray<FContactPoint>::Reset` 后的 `Max()`，当前二进制缺该字段。统一 Gate 先重建并重采，再用 `tools/import/import_mesh_contact_capacity.py --facts <source-facts.json> --native <native> --output src/als/settings/mesh_contact_capacity.rs` 生成目标容量并保存原始读数、watchdog 与身份。容量测试、编译、消融和 Playground 均待统一 Gate。
