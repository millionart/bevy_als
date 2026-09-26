# 资源目录

资源与蓝图按源模块放在同一棵树中：

- `ALS/`：角色、动画蓝图、Control Rig、物理资产、动画、曲线、OverlayObjects 与脚步材质。
- `ALSCamera/`：相机模型、动画蓝图、组件设置和曲线。
- `ALSExtras/`：Core/Environment/Levels/UI 的场景与逻辑定义、模型、材质、贴图和界面。
- `Engine/`、`Bevy/`：共用引擎内容和 Bevy 渲染扩展资源。
- `Validation/`：旧手工图及测试场景，不属于正式运行资源闭包。
- `docs/`：原始导出来源记录；目录迁移对应关系见 [资源归属说明](../docs/asset-layout.md)。

正式目标路径不使用 `generated/` 前缀。动画蓝图转换器默认按源 package path 安装，安装清单标明源身份和文件所有权。

原始 glTF/bin、纹理及曲线源表保持原字节；只有目标描述、图及代码中的资源引用随布局更新。不提交商业源 uasset/umap/fbx。重新导出或增加资源必须先在当前清单登记，再使用仓库原生 source reader 与导入工具，并更新导出记录。
