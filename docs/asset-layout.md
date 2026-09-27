# 资源与蓝图的源归属

目标资源树按 ALS-Refactored 的 Content 组织，模型、动画、曲线和蓝图目标文件属于同一个源模块。Rust 实现仍按 `src/README.md` 的 Source 归属组织。

| 资产根 | 对标与内容 |
|---|---|
| `assets/ALS/` | Character、Character/AnimationInstances、Animations、Data、OverlayObjects、Material；包括角色网格、骨架、PA_Als、CR_Als、动画蓝图和角色逻辑配置 |
| `assets/ALSCamera/` | SKM_Als_Camera、AB_Als_Camera、Data，以及相机组件/调试映射 |
| `assets/ALSExtras/` | Levels、Environment、UI；包括 Playground、场景逻辑配置、环境模型/材质和 W_Als_Hud/OverlayModeMenu 的目标定义 |
| `assets/Validation/` | 旧手工图、旧演示场景和作者性审计夹具；正式资源不引用它们 |

对应关系直接使用源 package path，例如 `/ALS/ALS/OverlayObjects/Bow/SKM_Als_Bow` 落在 `ALS/OverlayObjects/Bow/SKM_Als_Bow.gltf`，`/ALS/ALSCamera/AB_Als_Camera` 的图落在 `ALSCamera/AB_Als_Camera/`。一个 uasset 导出的 glTF/bin 或多个图文件可以共用其源资产目录；不添加 `generated/` 安装前缀。

资源保留原导出名及大小写。目录按功能归属组织，其中源 `MaterialLibrary` 在目标命名为 `Material`；UE 引擎网格贴图按使用者归入 Extras 环境材质，源身份仍记录为 `/Engine/EngineMaterials/T_Default_Material_Grid_M`。

角色运行包装图使用 `ALS/Character/AB_Als.runtime.animgraph.ron`，关卡角色场景使用 `ALS/Character/B_Als.animscn.ron`；Camera/Bow 采用同样的源名加目标格式后缀。包装图仍承担已有的 Ragdoll/接口接线，转换后的源图保持独立。安装清单仍记录源身份和所属文件，支持安全覆盖与清理，不用 generated 作为所有权依据。

## 原始导出与目录依赖

glTF、bin、贴图、曲线源表、BlendSpace 源定义和 Playground 原始 actor 表只移动位置，保持字节不变。Landscape 的 bin 和三张 glTF 导出贴图与 `Landscape1.gltf` 同处 `ALSExtras/Levels/L_Als_Playground/`，保留原有相对 URI。目标 RON/JSON/着色器与源码中的资源引用由迁移及生成工具更新。

当前真实角色仍使用独立 Roll slot sampler，因此 `RollSlot.animgraph.ron` 保留在 `ALS/Character/AnimationInstances/AB_Als_Grounded/`。其余旧手工图迁入 Validation，继续用于尚未执行的作者性审计；不会把历史夹具当作当前运行蓝图。

BF129.4 迁移基线为 1071 个跟踪文件，移除 4 个空目录占位。350 个原始载荷有逐字节保留要求，158 份 glTF 的外部 URI 保持不变；正式资源没有指向 Validation 的边。BF129.5 进一步将两份脚印 shader 归入 ALS、默认网格贴图归入 ALSExtras，并移除空的 Engine/Bevy 目录。

## 材质与 shader

| 使用对象 | 当前实现 |
|---|---|
| 角色 | 使用 Bevy 内置 `StandardMaterial`/PBR；颜色、粗糙度和反射参数在 [`character_assembly.rs`](../src/als/character/character_assembly.rs) 的 `make_als_character_material`/`make_als_main_material` 中组装，没有独立角色 WGSL |
| Overlay 道具 | [`overlay_object.rs`](../src/als/overlay_objects/overlay_object.rs) 根据 `ALS/Character/overlay_actor.runtime.json` 创建 `StandardMaterial`，复用 Bevy PBR |
| Playground 环境 | [`playground_materials.rs`](../src/als_extras/environment/playground_materials.rs) 定义材质扩展，shader 为 [`playground_tiles.wgsl`](../assets/ALSExtras/Environment/Materials/playground_tiles.wgsl)；默认网格纹理与 shader 同目录 |
| 脚印及接收表面 | [`ALS/Material/Footprint`](../assets/ALS/Material/Footprint) 中的 `packed_decal.wgsl`、`packed_decal_receiver.wgsl` 与 `T_Als_Footprint_M.tga`；Rust 绑定在 [`projected_decal.rs`](../src/bevy_ext/projected_decal.rs)，脚步事件在 [`footprint.rs`](../src/als/notifies/footprint.rs) |

内置 PBR shader 由 Bevy 依赖提供，不需要复制到项目资产树中。这里记录的是当前实际材质实现，完整的移植等价性以对应任务的 source Gate 为准。

## 转换与验收

全部 15 个已安装动画蓝图根及递归依赖必须从本地 uasset 重读并重生成；完整产物比对区分路径/包装标题变化与语义变化。移除 generated 命名不允许修改算法、源节点身份、时序、坐标或数值。既有生成器用目标图路径派生的辅助节点/条件上下文 ID 会随目录改变；这些 ID 必须双向唯一地重映射，所有边和布局一并核对，且不得改变原生源 GUID 或遗留旧 ID 引用。生成规则和资源导入器都写入新的正式目录，避免下次导入恢复旧布局。

正式项目可选择 ALS、ALSCamera、ALSExtras 模块资源；移除某个模块时也应停用对应插件/入口及其调用。Validation 与导出/审计说明不属于正式运行依赖，不需要复制到正式项目。
