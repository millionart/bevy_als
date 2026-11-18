# bevy_als

面向 Bevy 0.20.1 的 ALS 风格高级运动插件。角色系统覆盖地面与空中运动、蹲起、转身、翻滚和攀爬。动画图、脚部 IK、布娃娃与起身、相机适配 [ALS-Refactored](https://github.com/Sixze/ALS-Refactored) v4.18。移植范围和已知差异见[差异报告](docs/parity-report.md)。

在本仓库用以下命令运行最小角色场景。

```sh
cargo run --locked --jobs 2 --profile play --example als_character
```

使用 WASD 移动、鼠标转向、Ctrl 蹲起、Shift 冲刺和空格跳跃。完整场景见[最小示例](tools/examples/als_character.rs)。

## 在自己的项目中使用

在宿主的 `Cargo.toml` 添加依赖。示例地面使用 Avian 的 `Collider`，因此宿主也声明 `avian3d`。

```toml
[dependencies]
bevy = "0.20.1"
bevy_als = { path = "../bevy_als" }
avian3d = { version = "0.8", default-features = false }
```

也可以使用 `git = "https://github.com/millionart/bevy_als"` 和仓库中可取得的 `rev`。版本兼容以本项目 [Cargo.toml](Cargo.toml) 中的 Bevy、Avian、Enhanced Input 与动画依赖为准。

1. 将同一版本的 `ALS/`、`ALSRuntime/` 和 `ALSCamera/` 目录放入宿主的 Bevy 资产根，保留内部相对路径。`DefaultPlugins` 的 `AssetPlugin.file_path` 可以指定其他资产根。图、动画、相机、脚印和玩法标签共用这个根。
2. 在 `DefaultPlugins` 后注册 `bevy_als::AlsPlugin`。在启动系统中调用 `spawn_als_character`，传入生成位置和 `AlsCharacterBlueprint`。`spawn_input` 决定是否添加默认输入，`spawn_camera` 决定是否生成 `Camera3d`。调用示范见[最小示例](tools/examples/als_character.rs)。
3. 给角色提供带 `AlsCollisionLayer::Default` 的 Avian 地面。需要表面查询的地面同时声明 `Visibility`。使用角色上的 `AlsStateRequest` 请求步态、蹲姿或 Overlay 等状态。资产加载和角色出生在运行中完成。

`AlsPlugin` 负责注册 Tnua、动画图、玩法标签和 ALS 的系统，复用宿主已注册的 `EnhancedInputPlugin`。如果宿主需要预先配置 Avian，在 ALS 之前添加 `AlsPlugin::physics_plugins()`。这个组使用 `PostUpdate` 和 ALS 的碰撞 hooks。插件接入使用桌面 Bevy 的文件资产根。

宿主若要生成 Overlay 道具，可另注册 `bevy_als::overlay_object::OverlayObjectPlugin` 和 `bevy_als::extensions::overlay_materials::OverlayMaterialsPlugin`，两者分别负责道具与材质接线。

## 自定义 Overlay

在资产根下的 `ALS/Data/als_gameplay_tags.json` 登记新标签（例如 `Als.OverlayMode.CustomRifle`），并在 `ALS/Character/overlay_actor.runtime.json` 添加同名映射。可复制现有条目，再选择生成图、混合时间和物件配置。源导入器会补齐映射所需的 Tag 注册，并保留已有自定义标签。

重新启动 Playground 后，Q 菜单会从 Tag 管理器枚举标签。代码入口使用 `OverlayState::new` 和 `AlsStateRequest.overlay_state`，库由 `OverlayLibraryHandle` 选择。图和物件共同消费加载后的库，更新同一标签的配置也会刷新物件及图绑定。

自定义 LocomotionMode 使用 `LocomotionState::new("Als.LocomotionMode.Climbing")`，通过 `AlsStateRequest.locomotion_state` 传入；RON 数据使用 `Named("Als.LocomotionMode.Climbing")`。原有 `LocomotionState::Custom` 保留物理控制含义并映射为空 Tag。命名入口保留身份，不自动实现对应的物理动作或动画分支。

## 布娃娃物理边界

ALS-Refactored 使用 UE Chaos。本项目使用 Avian 的原生碰撞与接触求解，几何接触由 Parry 计算。布娃娃的接触点、法线、处理顺序、摩擦结果及碰撞后的姿态和轨迹可以与源工程不同。布娃娃地面球体扫掠采用 Avian 原生查询的命中距离。

布娃娃仍须正常倒下、与场景碰撞并起身。碰撞体、关节和必要的碰撞过滤继续提供这些行为。行走、上下台阶、攀爬、脚部 IK 及其他普通角色移动和动画功能继续按 ALS 的既定目标验证。这项豁免仅适用于布娃娃碰撞的引擎差异。

每个刚体的 CCD 开关映射到 Avian `SweptCcd` 的过滤配置。当前 `PA_Als` 的 21 个刚体使用空过滤，关闭扫掠 CCD。开启的刚体使用 Avian 的原生线性扫掠。

## Playground 与维护

`playground` feature 启用原场景、HUD 和自动验收工具。`run_manual.ps1` 自动带上该 feature，Windows 可双击 `run_manual.bat`。

```powershell
pwsh -NoProfile -File ./run_manual.ps1
pwsh -NoProfile -File ./als_graph_editor.ps1
pwsh -NoProfile -File tools/run_animation_visual_validation.ps1 -Case startup -CaptureCount 0 -FunctionalOnly
cargo test --workspace --locked --jobs 2 --features playground -- --test-threads=1
```

[工具说明](tools/README.md)记录 PowerShell、Python 和 UE 原生导入器的配置。[源码目录](src/README.md)、[现行清单](CHECKLIST.md)、[已归档清单](docs/archive/archivedchecklist.md)与[证据索引](docs/audit/README.md)提供维护入口。手动调试显示类别保存于本地 `tools/tmp/saved/als_debug.ron`。

## 许可

本仓库自有代码采用 [MIT](LICENSE-MIT) 或 [Apache-2.0](LICENSE-APACHE)。`assets/` 含从 ALS/UE 导出的模型、动画和贴图，使用这些资产时适用其原始许可。[资产来源](assets/docs/export-manifest.md)
