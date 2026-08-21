# assets/docs/export-manifest.md — 导出资产清单

> 每次从 ALS-Refactored 工程导出占位资产，必须在此登记：资产名、源 uasset 路径、
> 导出格式、日期。禁止提交 ALS 商店原始 uasset（见 `.gitignore` 与 agents.md 规则 5）。
> 导出方式与 MCP 连接步骤见 `docs/env.md`（隐私文件）。

## 约定

- 目录：
  - `assets/models/` — UE5 Manny 蒙皮网格 + 骨骼骨架（glTF）
  - `assets/animations/` — 动画组 Grounded/Air/Actions/Transitions/TurnInPlace/RotateInPlace/Overlays/View/Base（glTF/JSON 中间格式）
  - `assets/graphs/` — 动画图 RON（本仓库自建，非 UE 导出，不在此登记）
  - `assets/curves/` — 关键曲线数值表（RON）
- 源工程：ALS-Refactored（`Content/ALS/...`，v4.18，UE 5.8）
- 导出通道：Unreal MCP（Interchange glTF 导出），见 `docs/env.md`

## 登记记录

| 日期 | 资产 | 源 uasset 路径 | 导出格式 | 目标路径 |
|---|---|---|---|---|
| （暂无 — M0.5 一次性批量导出后填写） | | | | |
