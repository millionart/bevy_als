# BF153.01 — 统一 Cargo 构建目录

起点为 `6219f761`。手动入口曾覆盖 `CARGO_TARGET_DIR`，新 worktree 曾复制 `target` 和 `target_game/play`；图编辑器和普通 Cargo 命令则使用 Cargo 默认目录。[修复前反例](before.txt)包含默认、环境变量、Cargo 配置和 worktree 两套缓存的失败。

手动入口现在交由 Cargo 选择构建目录，保留原有 profile、任务数和退出码；Windows 不再删除共享目录中的 PDB。worktree 只复制 `target`，仍排除增量和临时目录。历史审计中会调用 Cargo 或读取编译缓存的执行脚本已改用当前目录；历史报告和已保存的 `rustc` 参数快照不改写。`.gitignore` 保留旧目录规则，避免其他工作区尚未迁移的缓存被意外加入 Git。

本机旧目录 7 个顶层项目均在同一文件系统内移动到 `target`，目标无覆盖；两个同名元数据文件放在 `target/legacy-target-game-metadata`。移动前后 inode 相同，普通文件额外核对 SHA-256；[迁移清单](cache-migration.json)记录每个目标。`target_game` 已移除，原 `target` 中的资料保留。现有部分 Cargo depfile 仍含旧路径，首次构建可能重新编译相关缓存。

验证：

- `python3 -B tools/tests/test_platform_tools.py -v`：15 项通过，含实际 Cargo 最小项目的默认目录、环境变量和配置文件三种场景，以及 worktree 和模拟 Windows PDB 反例。[结果](platform-after.txt)、[watchdog 报告](platform-watchdog.json)
- `python3 -B docs/audit/bf153/01/ablation.py`：在独立临时夹具恢复旧目录覆盖、PDB 删除和第二缓存复制，分别触发失败；原脚本字节不变且对照通过。[结果](ablation.json)
- `pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -SelfTest`：六项 watchdog 自测通过。[结果](watchdog-self-test.txt)
- [静态检查](static.txt)：13 个历史执行脚本路径及语法、迁移后的依赖元数据、Cargo metadata 默认目录与 PowerShell 解析通过。平台回归还暴露一个原有测试仍读取旧报告目录；[修复前结果](platform-before.txt)与通过结果保留。

未重建或运行完整 Bevy Playground，也未在 Windows 真机运行。历史 `rustc` 参数快照保留原路径，重放这些旧快照需要先处理路径和当时的依赖版本；本项不声明旧审计已重新验收。
