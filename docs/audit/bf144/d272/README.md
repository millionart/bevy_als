# BF144-D272 冷相机视点的输入朝向

`cold.request.json` 通过仓库 source reader 重新读取本地角色；`cold.native.json.gz` 的原始/压缩长度与 SHA 在 `native-identities.json`，`cold-snapshot.json` 记录实际 controller 60°、manager 存在、cache 时间0、viewtarget 角色及实际 getter 0°。`source-facts.json` 保存 fresh UE/ALS 调用链、修改前目标路径和 `.442c0/.442c1/.442c2` 证据身份。原始压缩结果已按解压后 SHA 核对。

目标在玩家角色、关联有效 camera source 且 camera 活跃时记录该 source 是否曾发布视点；首次发布前缺少视点时，移动输入使用角色世界朝向。首次发布后视点失效、相机停用、manager 不存在、角色没有玩家资格等情况沿原回退路径；source 更换时重建状态。此状态代表目标是否发布过该相机源，并不声称获得了源 PlayerCameraManager 的原生 cache 时钟，也不将 camera-off 等同源冷缓存。

已写发布/停用/替换/失去玩家资格的生命周期、actor/control 60°差异、已发布视点与无 manager 控制，并接入实际角色输入系统。全部静态修复和 Bevy 迁移完成后再运行这些 focused 测试、状态消融及原 Playground Gate；移除冷分支或首次发布状态应分别被不同反例拒绝，恢复源码字节后在同一 target 重编复验。以上测试及消融尚未运行。
