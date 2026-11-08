# BF144-D64 Mantle 目标的实时可移动性

fresh ALS RMS 每次准备都重新调用 `UseRelativeLocation`；UE 依据当前目标 primitive 的 Mobility 判断，而初始 root warp 坐标保持原样。fresh High Montage 输出与 D57 完全相同，复用其 Git 文件；请求、reader 报告、身份和 ALS/UE 源码片段在本目录。

目标每帧读取命中实体所属刚体的 Static/Kinematic/Dynamic 状态，再决定当前如何解释已存 warp 坐标，不重新换算已存坐标；目标没有刚体信息时保留原选择。已写两种切换、未切换控制及加载角色对照；测试、消融和动画 Gate 待统一验证。组件独立 Mobility 与父刚体状态不一致的组合未在本项复测；旧 `target/bf144/19/mobility/` 原始报告缺失。
