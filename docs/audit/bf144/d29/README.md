# BF144-D29 基座携带时的胶囊世界旋转

fresh UE `UpdateBasedMovement` 使用组件当前世界 quat 进行基座位移扫掠；角色 CDO 为 `bIgnoreBaseRotation=True`、`bFastAttachedMove=False`，前者不移除角色已有的倾斜，后者保留碰撞。fresh 原生等价坐标系探针测得 60° 胶囊和平墙命中距离约 113.038 cm，直立对照为 165 cm。

目标基座携带查询使用角色实际世界旋转，位置仍按原有父空间规则写回。已写加载角色直接/父级旋转、直立和无墙八组对照，并保留基座原有回归。历史目标报告 `target/bf144/10/carry_rotation/` 已缺失；测试、消融、图审计和真实 Playground Gate 待全部静态修复及 Bevy/依赖升级后执行。
