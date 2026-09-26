# 工具入口

| 目录或入口 | 保留用途 |
|---|---|
| `run_ue_source_reader.ps1`、`build_ue_source_reader.ps1`、`ue_graph_source_reader/` | 从本地 UE/uasset 提取图、属性与递归依赖；reader 带进展 watchdog 和 `-SelfTest` |
| `animgraph_authoring/` | AnimBP/Control Rig 转换、完整产物比较、源闭包审计及夹具；其中合同与地图仍被图审计 TODO 引用 |
| `import/` | 源事实转为独立 runtime 资产、单位/坐标换算、材质通道和图合同审计 |
| `export/` | 网格、材质、纹理、地形等资产导出；`export_playground_actors.py` 生成网格导出器消费的 actor 清单 |
| `uasset_curve_export/`、`uasset_root_export/` | 曲线、root track 的导出与验证；`probe_animdatamodel.py` 仍是可独立使用的原生格式诊断入口 |
| `run_animation_visual_validation.ps1`、`validate_animation_visual_evidence.py`、`validate_decal_evidence.py` | 当前 manual Playground 的公共运行与报告验收 |
| `validate_*`、`verify_*`、`*animgraph*`、`normalize_*` | 当前节点、上下文、布局、资产边界和编辑器审计 |
| `open_ue_asset_editor.ps1`、`ue_open_asset.py` | 本地 UE 资产编辑器辅助入口，不使用 MCP |
| `new-worktree.ps1` | 工作区及构建缓存准备 |

`import/probe_anim_graph_nodes.py`、`probe_property_access.py`、`probe_gltf_curves.py` 接受显式资产参数，仍可检查原生属性或导出曲线。没有其他脚本调用一个 CLI，不等于它已经无用。

2026-09-26 删除了 UE MCP client 及其完整调用链（输入审计、旧相机导出、拓扑/截图和五个 shell 探测脚本），当前取证统一使用原生 reader。相机曲线的目标转换使用 `import/import_curve_float.py`、`import/import_debug_curves.py` 和 `import/camera_curve_conversion.py`。

同批删除六个早期 Interchange API 试验脚本、没有消费者的 `probe_scene2.py`，以及反复改写旧 Playground parity 文件的 `patch_parity_ron.py`、`test_lighting_only.py`。实际导出器、原始资产、生成图、归档证据和本地配置保留。
