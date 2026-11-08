# BF144-D100b2 目标接触子形体身份

Parry 对 compound 和 triangle mesh 的接触 manifold 保留对应子形体编号；Avian 的接触转换只复制点、法向和局部 feature，随后可能删除点或整个 manifold，但保持剩余 manifold 次序。`source-facts.json` 保存双方对应代码及文件哈希；fresh 角色与环境原生证据复用 `../d100b/`。

新增身份恢复函数对调用方提供的相同 collider、世界姿态和预测距离再次查询 Parry，使用 Avian 同一公式计算世界点，再以法向、保留点、feature 和原次序匹配。若存在多个可行匹配且子形体身份不同，返回缺失，不能猜测。编号是目标子形体身份；mesh 的目标三角形编号尚需与源几何和 BVH 身份对应。

已编写双向 compound/mesh、点与 manifold 裁剪、无法匹配和歧义的 focused 测试。该函数尚未接入求解器，也未解决接触顺序；后续 D100b3/b4 必须提供准确预测距离并处理无法唯一对应的情况。编译、测试、消融和真实入口运行均待升级后的统一 Gate。
