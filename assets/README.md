# assets/ 目录约定（M0.3）

| 目录 | 内容 | 来源 |
|---|---|---|
| `models/` | UE5 Manny 蒙皮网格 + 骨骼骨架（glTF） | UE 导出（M0.5） |
| `animations/` | ALS 动画组 clip（glTF/JSON 中间格式） | UE 导出（M0.5） |
| `graphs/` | bevy_animation_graph 图资产（RON） | 本仓库自建 |
| `curves/` | ALS 关键曲线数值表（RON） | UE 导出（M0.5） |
| `docs/` | `export-manifest.md` 导出清单 | 每次导出登记 |

规则（agents.md 规则 5）：

- 占位资产在 M0.5 一次性批量导出，此后不得零散补导；确需补充须先在 `CHECKLIST.md` 增补条目。
- 禁止提交 ALS 商店原始 uasset/umap（`.gitignore` 已排除 `*.uasset`/`*.umap`/`*.fbx`）。
- 每次导出在 `docs/export-manifest.md` 登记：资产名、源 uasset 路径、导出格式、日期。
