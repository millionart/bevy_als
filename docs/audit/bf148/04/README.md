# BF148.04 Mesh Space 骨骼遍历

本地 `AnimationRuntime.cpp` 1171–1176 从已构建的父旋转向子节点构建 mesh 旋转，1202–1207 反向恢复 local。2706–2723 组件空间变换同样读取已经建立的父变换。`AnimNode_LayeredBoneBlend.cpp` 306–322 按各自 flag 选择 mesh 旋转、root 旋转与 scale。Head、Layering 原生闭包共 16 个图，其中五个 Mesh Space Additive 和九个 Layered Bone Blend。原生资产、reader 参数及摘要见 [native.json](native.json)。

三处 `hierarchy_order` 与三处局部旋转读取具有相同函数体，忽略类型路径与局部参数名后的前后三对指纹见 [helper-equivalence.json](helper-equivalence.json)。两个函数沿用 additive 中的实现，另外两处导入。遍历 Root、前置祖先和可用 rotation → ref local → identity 的次序保持同一合同。Pose Difference 的乘法与逆、Layered Blend 的权重、SourcePose 与架构归一化各按原路径求值。

[作者性清单](../../../../tools/animgraph_authoring/bf148_mesh_hierarchy.authoring.json) 核对真实 Head 图、原生读数与运行图资产边界。单测覆盖三个 mesh-space 模块和原生分层节点。公共入口用 startup、head 和 standing_stop 验证实际姿态，后者也消费实际角色的 Montage 差分。

同 target 34 项定向通过。startup、head、standing_stop 均通过功能 Gate，原始报告长度和 SHA-256 见 [runtime.json](runtime.json)。
