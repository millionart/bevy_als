# bevy_als — agents.md（执行规则）

用 Bevy ECS 复刻 [ALS-Refactored](https://github.com/Sixze/ALS-Refactored)（v4.18，UE 5.8）的高级运动系统：
以 `bevy_animation_graph`（替代 UE AnimBP）+ `bevy-tnua`（替代 UE CharacterMovement/KCC）为基座，
按需扩展这两个插件，用 Bevy/ECS 思路重建，不背负 UE 技术债。暂不包含网络同步。

**执行内容一律以 [`CHECKLIST.md`](CHECKLIST.md) 为准**（模块清单 + 各模块 Gate）；本文件只规定执行纪律。

## 文档索引

| 文件 | 内容 |
|---|---|
| `agents.md` | 执行规则（本文件） |
| `CHECKLIST.md` | 执行清单：基础事实、M0–M11 模块、每模块 Gate、附录 |
| `goal.md` | goal 执行入口说明 |
| `docs/env.md` | 本地隐私环境（gitignore）：路径、引擎版本、Unreal MCP 连接步骤 |

## 执行规则

1. **自上而下**：严格按 `CHECKLIST.md` 模块顺序执行；每个模块内的清单项自上而下逐项完成，不做跨模块跳跃。
2. **每项一提交**：实现一个清单项 → 自测通过 → `git commit -m "M<编号>: <条目摘要>"` → 在 `CHECKLIST.md` 勾选该项。
3. **Gate 约束**：每个模块最后一个清单项是本模块验收 Gate。Gate 未通过不得开始下一模块；后续发现前序模块缺陷时回退修补并重新过 Gate。
4. **插件扩展边界**：对两个基座插件的通用能力扩展（如 curve、IK 节点）优先做成可回馈上游的独立改动，先落在本仓库 `crates/` 下扩展 crate 中，保持 fork 可 rebase。
5. **资产占位**：动画/模型资产通过 Unreal MCP 从 ALS-Refactored 工程导出（配置步骤见 `docs/env.md`），导出的占位资产放 `assets/` 并登记 `assets/docs/export-manifest.md`；禁止把 ALS 商店原始 uasset 提交进仓库。
6. **禁止**：引入 UE 式继承树/反射式蓝图思维；在无 Gate 记录时宣称模块完成。
