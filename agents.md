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

1. **自上而下**：严格按 `CHECKLIST.md` 模块顺序执行，不做跨模块跳跃；模块内按清单自上而下推进，其中无数据依赖的清单项可按规则 6 并行。
2. **每项一提交**：实现一个清单项 → 自测通过 → 在 `CHECKLIST.md` 勾选该项 → 连同勾选一次提交（`M<编号>: <条目摘要>`），保持工作区无未提交残留。
3. **Gate 约束**：每个模块最后一个清单项是本模块验收 Gate。Gate 验证必须留可复查证据（命令输出摘要/截图路径，写入该次提交说明）；Gate 未通过不得开始下一模块；后续发现前序模块缺陷时回退修补并重新过 Gate。
4. **插件扩展边界**：基座插件的能力扩展优先经其公开扩展点在本仓库 `crates/als-ext` 实现（如 curve、ControlRig 等价 IK），不修改 fork；仅当公开扩展点确实不足时才修改 fork，且 fork 改动必须独立小步提交、可回馈上游。任何时候禁止 vendor 拷贝插件源码。
5. **资产占位**：动画/模型/曲线资产通过 Unreal MCP 从 ALS-Refactored 工程导出（配置步骤见 `docs/env.md`），在 M0.5 一次性批量完成，此后不得零散补导——确需补充须先在 `CHECKLIST.md` 增补明确条目再执行。导出物放 `assets/` 并登记 `assets/docs/export-manifest.md`；禁止把 ALS 商店原始 uasset 提交进仓库。
6. **并行开发（按需优先）**：同一模块内无数据依赖的清单项，优先拆分给多个 subagents 并行执行（如资产导出与代码实现、相互独立的系统/测试）；不同 subagent 不得修改同一文件，存在写入交集时退回串行。跨模块仍受规则 1 约束不得并行。每个 subagent 只负责单一明确产出，代码合并、提交、勾选与 Gate 验证由主代理统一串行收口。
7. **禁止**：引入 UE 式继承树/反射式蓝图思维；在无 Gate 记录时宣称模块完成。
