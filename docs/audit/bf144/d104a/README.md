# BF144-D104a 源接触几何复用

源 `TryRestoreManifold` 对比上次窄相位与当前两形体的相对位置及旋转；位移逐分量比较，旋转使用相对四元数点积。满足条件后，以初始 shape0 接触点投影到 shape1 接触面，切向误差超限则禁用点；有效点不足 `min(原点数, 4)` 时拒绝复用。shape0 点和最初的两侧局部点继续保留，shape1 点及 phi 在写回时按源单精度厘米量化。

`box.native.json.gz` 和 `mesh.native.json.gz` 分别保存 8 组 fresh box 与 8 组凸包/三角网格两帧结果，含球体和胶囊不复用控制；请求、reader watchdog、哈希和源环境资产身份见 `native-identities.json`。`restore.native-samples.json` 可用 `python3 tools/import/export_contact_restore_probe.py --source docs/audit/bf144/d104a/box.native.json.gz docs/audit/bf144/d104a/mesh.native.json.gz --output docs/audit/bf144/d104a/restore.native-samples.json` 重建。

`source-facts.json` 保存源条件和容差定义；执行 `python3 tools/import/import_contact_restore_settings.py --facts docs/audit/bf144/d104a/source-facts.json --output src/als/settings/contact_restore.rs` 可重建目标配置。已编写全部原生样本和空接触、旋转、禁用点控制。实际身份缓存与碰撞生命周期属于 D104b；测试、消融和 Playground 尚未运行。
