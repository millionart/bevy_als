# bevy_als

本项目是单个 Cargo 库，公开入口为 `bevy_als::AlsPlugin` 与 `spawn_als_character`。角色逻辑在 `src/als`，相机在 `src/als_camera`，示例场景在 `src/als_extras`。底层扩展保留在对应的 `src/*_ext` 模块。

- 保持公开插件、已安装资产、RON 反射身份和宿主调度兼容。先使用 Bevy 和依赖的公开扩展点。
- 外部 Bevy 引擎仓库对代理只读。引擎文件、Git 数据、文件和目录权限均由用户管理。项目需要的补充放在本仓库。
- 缺陷先用最小反例复现，确认根因后修复。按受影响的系统、资产与调用方选择回归。大范围 Cargo 命令使用 `--jobs 2`。构建和清理从项目根执行。
- Rust 回归的 JSON 数据统一放在 `src/tests/fixtures/`，按模块最多分一层，以行为和数据命名。保持测试的输入、门槛与断言。重现用例无需保留原始捕获、日志或本机路径。
- `cargo test --locked --jobs 2 --features playground -- --test-threads=1` 运行完整回归。`cargo run --locked --jobs 2 --example als_character` 运行最小示例。`cargo run --locked --jobs 2 --features playground --example manual_playground` 启动完整 Playground。
- 提交中使用相对资产路径，保留资产来源的许可。构建与开发者本机配置由 Git 本地忽略规则管理。
