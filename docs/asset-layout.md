# 资源与蓝图的源归属

目标资源树按 ALS-Refactored 的 Content 组织，模型、动画、曲线和蓝图目标文件属于同一个源模块。Rust 实现仍按 `src/README.md` 的 Source 归属组织。

| 资产根 | 对标与内容 |
|---|---|
| `assets/ALS/` | Character、Character/AnimationInstances、Animations、Data、OverlayObjects、MaterialLibrary；包括角色网格、骨架、PA_Als、CR_Als、动画蓝图和角色逻辑配置 |
| `assets/ALSCamera/` | SKM_Als_Camera、AB_Als_Camera、Data，以及相机组件/调试映射 |
| `assets/ALSExtras/` | Levels、Environment、UI；包括 Playground、场景逻辑配置、环境模型/材质和 W_Als_Hud/OverlayModeMenu 的目标定义 |
| `assets/Engine/` | 引用的 UE 引擎内容，如 EngineMaterials 的默认网格纹理 |
| `assets/Bevy/` | Bevy 扩展自己的通用着色器，区别于源项目资源 |
| `assets/Validation/` | 旧手工图、旧演示场景和作者性审计夹具；正式资源不引用它们 |

对应关系直接使用源 package path，例如 `/ALS/ALS/OverlayObjects/Bow/SKM_Als_Bow` 落在 `ALS/OverlayObjects/Bow/SKM_Als_Bow.gltf`，`/ALS/ALSCamera/AB_Als_Camera` 的图落在 `ALSCamera/AB_Als_Camera/`。一个 uasset 导出的 glTF/bin 或多个图文件可以共用其源资产目录；不添加 `generated/` 安装前缀。

角色运行包装图使用 `ALS/Character/AB_Als.runtime.animgraph.ron`，关卡角色场景使用 `ALS/Character/B_Als.animscn.ron`；Camera/Bow 采用同样的源名加目标格式后缀。包装图仍承担已有的 Ragdoll/接口接线，转换后的源图保持独立。安装清单仍记录源身份和所属文件，支持安全覆盖与清理，不用 generated 作为所有权依据。

## 原始导出与目录依赖

glTF、bin、贴图、曲线源表、BlendSpace 源定义和 Playground 原始 actor 表只移动位置，保持字节不变。Landscape 的 bin 和三张 glTF 导出贴图与 `Landscape1.gltf` 同处 `ALSExtras/Levels/L_Als_Playground/`，保留原有相对 URI。目标 RON/JSON/着色器与源码中的资源引用由迁移及生成工具更新。

当前真实角色仍使用独立 Roll slot sampler，因此 `RollSlot.animgraph.ron` 保留在 `ALS/Character/AnimationInstances/AB_Als_Grounded/`。其余旧手工图迁入 Validation，继续用于尚未执行的作者性审计；不会把历史夹具当作当前运行蓝图。

本轮迁移清点：1071 个跟踪文件，移除 4 个空目录占位；其余分为 ALS 822、ALSCamera 26、ALSExtras 59、Validation 154、Engine 1、Bevy 2 和 3 个说明文件。350 个原始载荷有逐字节保留要求，158 份 glTF 的外部 URI 均能在新位置保持不变；2199 条已识别目标资源引用中，正式资源没有指向 Validation 的边。

## 转换与验收

全部 15 个已安装动画蓝图根及递归依赖必须从本地 uasset 重读并重生成；完整产物比对区分路径/包装标题变化与语义变化。移除 generated 命名不允许修改算法、节点身份、时序、坐标或数值。生成规则和资源导入器都写入新的正式目录，避免下次导入恢复旧布局。

正式项目可选择 ALS、ALSCamera、ALSExtras 模块资源；移除某个模块时也应停用对应插件/入口及其调用。Validation 与导出/审计说明不属于正式运行依赖，不需要复制到正式项目。
