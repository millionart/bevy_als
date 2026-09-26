# goal.md — bevy_als 执行目标（供 goal 执行时引用）

> 执行体：`agents.md` 是执行纪律，`CHECKLIST.md` 是唯一权威执行清单。本文件只负责"如何启动与推进"。

## 目标

按 `CHECKLIST.md` 的当前依赖顺序，以"模块内唯一清单自上而下、每项一提交一勾选"的方式，持续推进 ALS-Refactored（UE 5.8, v4.18）的 Bevy ECS 复刻；已完成批次统一在归档索引中保留证据。

## 启动方式

1. 通读 `agents.md`（执行规则）与 `CHECKLIST.md`（当前清单）。
2. 从 `CHECKLIST.md` 第一个未勾选的清单项开始执行；无待执行项时先新增明确条目。
3. 每完成一项：自测 → 在 `CHECKLIST.md` 中勾选 `[x]` → 连同勾选一次提交（`M<编号>: <条目摘要>`）。
4. 遇到 Gate 项：完整执行 Gate 验证，证据（命令输出/截图路径）写入该次提交说明后方可勾选；Gate 未过不得进入下一模块。
5. 需要本地路径/引擎信息时查 `docs/env.md`（隐私文件，勿提交）；UE 源事实使用仓库原生 uasset 读取器。
6. 占位资产已在 M0.5 一次性批量导出；后续不得零散补导，确需补充先在 `CHECKLIST.md` 增补条目再执行（导出步骤见 `docs/env.md`）。
7. 项目使用单一 Cargo 包；源码按 `agents.md` 与 `src/README.md` 的源模块/引擎扩展分区，经基座公开扩展点实现。ALS、ALSCamera、ALSExtras 与底层扩展在根 src 中分别归位，资产反射类型名保持兼容。确需修改插件本体时才动 fork，且必须独立小步提交推送、可回馈上游；ALS 状态、资产选择和角色生命周期留在对应 ALS 分区。

## 验收

- 当前 `CHECKLIST.md` 的全部清单项（含 Gate）勾选完成或按规则归档；
- `cargo test --workspace` 全绿；
- `docs/parity-report.md` 存在并记录与 ALS 的差异。

## 边界

- 不做网络同步；不做 ALSExtras AI；音频/特效只留挂钩点。
- 不提交 ALS 商店原始资产与任何隐私路径。
