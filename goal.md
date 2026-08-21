# goal.md — bevy_als 执行目标（供 goal 执行时引用）

> 执行体：`agents.md` 是执行纪律，`CHECKLIST.md` 是唯一权威执行清单。本文件只负责"如何启动与推进"。

## 目标

按 `CHECKLIST.md` 的模块顺序（M0→M11），以"模块内唯一清单自上而下、每项一提交一勾选"的方式，把 ALS-Refactored（UE 5.8, v4.18）的运动系统以 Bevy ECS 复刻到本仓库，最终通过 M11 最终 Gate。

## 启动方式

1. 通读 `agents.md`（执行规则）与 `CHECKLIST.md`（基础事实与模块清单）。
2. 从 `CHECKLIST.md` 第一个未勾选的清单项开始执行（首次为 M0.1）。
3. 每完成一项：自测 → `git commit -m "M<编号>: <条目摘要>"` → 在 `CHECKLIST.md` 中勾选 `[x]`。
4. 遇到 Gate 项：完整执行 Gate 验证并留证据（命令输出/截图路径）后才勾选；Gate 未过不得进入下一模块。
5. 需要本地路径/引擎/MCP 连接信息时查 `docs/env.md`（隐私文件，勿提交）。
6. 需要动画/模型占位资产时，通过 Unreal MCP 从 ALS-Refactored 工程导出（步骤见 `docs/env.md`），登记到 `assets/docs/export-manifest.md`。
7. 插件能力扩展直接在对应 fork 仓库实现并提交推送（curve 等进 bevy_animation_graph fork）；ControlRig 子集（two-bone/foot/pelvis/hand IK）在私有仓库 bevy_controlrig 实现并提交；bevy_als 本体保持纯净的 ALS 系统，仅做编排与消费。

## 验收

- `CHECKLIST.md` 中 M0–M11 所有清单项（含全部 Gate）勾选完成；
- `cargo test --workspace` 全绿；
- `docs/parity-report.md` 存在并记录与 ALS 的差异。

## 边界

- 不做网络同步；不做 ALSExtras AI；音频/特效只留挂钩点。
- 不提交 ALS 商店原始资产与任何隐私路径。
