# BF154.04 独立项目接入

`AlsPlugin` 指向现有 ALS 角色插件。默认入口安装 Avian 的 `PostUpdate` 组和原布娃娃碰撞 hooks。宿主可以提前添加 `AlsPlugin::physics_plugins()` 来配置同一组；入口检查阶段和 hook 类型。已有 Enhanced Input 继续使用宿主实例。玩法标签使用 Bevy 文件读取器的基准目录和宿主 `AssetPlugin.file_path`。

[消费方夹具](../../../../tools/tests/fixtures/als_consumer.rs)作为独立 Cargo 包的 `src/main.rs` 运行。宿主预装 Enhanced Input 和 ALS 物理组；资产根为 `game-assets`，工作目录没有 `assets`。夹具检查真实标签、角色和相机装配，再用键盘输入和 `AlsStateRequest` 验证蹲行。外层 watchdog 等待控制台的装配与移动进度。

消费方使用以下依赖。将 `path` 指向当前 checkout，使用 `cargo run --jobs 2 -- game-assets` 运行。`game-assets` 提供同一版本的 `ALS`、`ALSRuntime` 和 `ALSCamera` 资产目录。持久化的本次命令与产物身份见同目录报告。

[消费方运行](consumer-runtime.txt)加载了标准标签和真实资产，角色进入蹲姿并在键盘输入下移动 0.259 m。[资产根对照](consumer-default-path-red.txt)指向不存在的 `assets` 后在相同断言失败。旧入口[重复添加输入插件](plugin-duplicate-red.txt)失败。临时移除调度检查时，[错误调度不再被拒绝](plugin-schedule-ablation.txt)。按字节恢复并重编后 [3 项插件测试](plugin-restored.txt)通过。 [产物与输入身份](identity.json)

```toml
[dependencies]
bevy = "0.20.1"
bevy_als = { path = "../bevy_als" }
avian3d = { version = "0.8", default-features = false }
bevy_enhanced_input = "0.27"
bevy_gameplay_tag = { git = "https://github.com/millionart/bevy_gameplay_tag", rev = "30a41183ac0705e6b7b483d0a72f0ca70510327e" }
```
