# BF144-D246b1 三角网格双精度射线

fresh UE 对 Playground 使用的 `SM_Als_Cart` 复杂碰撞发出非整数水平坐标射线，原生 ImpactPoint 保留输入 X/Y，命中高度为 `55.695702670963016` cm；源 BVH 记录了六个候选访问及最终三角面。`mesh.request.json`、完整原生输出、选中三角顶点和资产哈希均保存在本目录。源网格的 backface culling 为 true，不能据此声称目标选面或全部碰撞语义已相同。

目标 Avian 的 scaled TriMesh 已提供 BVH、节点 F32 包围盒与叶三角索引。查询复用原 BVH，将节点边界和实际叶顶点精确提升到 F64，再用 Parry F64 slab/triangle 求交；只重建每次命中点，既不逐射线重建 BVH，也不新增全局形状缓存。单三角、双层最近三角、miss 与原盒体/混合过滤 fixture 已写，尚未执行。

碰撞顶点和物体变换仍来自 F32；共享边/顶点及 backface 选面属于既有 D162。球体、胶囊、compound 等仍走 F32 回退，留 D246b2；不能将源 Cart 读数称为目标运行结果。全量测试、消融/恢复、图审计、性能和公共 Playground Gate 待全部静态修复及 Bevy/依赖迁移后执行。
