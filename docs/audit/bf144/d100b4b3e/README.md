# D100b4b3e 网格顶点法线

原生启用顶点法线修正与边界面替换。源从原接触三角读取该 cooked 顶点的位置，按采集序检查所有共享该 ID 的三角。若所查三角的任一入边无邻面，取原接触三角的法线。否则用两条有向边形成棱柱边界，带符号平方与源容差乘边长平方比较，取首个满足两侧条件的三角法线。保持 phi、点类型和原 cooked 面号，重建第一支持点。棱柱命中时更新局部三角身份。

`tools/import/import_mesh_contact_vertex.py` 从 `source-facts.json` 提取源 double 容差到 `src/als/settings/mesh_contact_vertex.rs`。已写开放邻接回原面与闭合扇面进入邻棱柱的 focused 对照。编译、测试、消融和 Playground 待统一 Gate。
