# 原始 Root 与 Rig 反馈重采

本轮用封存的 `.457` 请求重采完整源 `B_Als_Character`。主序列含初始化及318普通帧，附加控制含初始化及2帧。新读出的原生probe数据段与已封存数据段逐字节一致。`reconstruct.py` 使用新读数前缀和已封存尾段复原218,683,762字节，`identity.json` 记录两个分段和完整SHA-256。

目标从正式可见 Playground 获取79冷启动及240实际活动帧，沿用原30Hz和两段移动输入。冷段79个前Rig姿态的F32投影及Root曲线精确。前两次完整Root投影精确，第三次从calf_l的Rig输出出现余差。活动阶段实际模式、输入及位置与原生配对，首移动步的旧速度量化见 `motion-plan-first-source-frame.json`。冷Rig的网格原点和足目标输入边界见 `cold-rig-first-input-differences.json`。

三份旧目标报告逐值核对了额外观察的被动性。`target-30hz` 记录基础阶段，`target-producers-30hz` 记录首八帧Rig外部量，`target-motion-plan-30hz` 在速度快照与转向之间读取计划。`observer.initial.rs`、`observer.producers.rs` 及 `observer.rs` 分别对应三次观察。`compare_target.py` 使用原生字段计算F32姿态投影、完整曲线和物理输入，保留冷段与活动段的各自首差。

`target-clock-restored-30hz` 使用同一观察和已恢复的角色时钟。首移动速度和Speed逐值一致，实际终点相同。首活动帧LayerLegs恢复，FeetCrossing余1 ULP，RotationYawOffset方向差与冷Rig输入余差继续追踪。正式恢复启动报告见 `docs/audit/bf144/d254a/runtime-recheck/`。

```sh
d=docs/audit/bf144/u07/source-root-history
mkdir -p target/bevy020-gates/u07-root-history
python "$d/reconstruct.py" target/bevy020-gates/u07-root-history/source-318.native.json
cp "$d/observer.rs" target/bevy020-gates/u07-root-history/observer.rs
git apply "$d/observer-hook.patch"
pwsh -NoProfile -File "$d/run.ps1" -Directory target/bevy020-gates/u07-root-history/replay
git apply -R "$d/observer-hook.patch"
python "$d/compare_target.py" target/bevy020-gates/u07-root-history/source-318.native.json target/bevy020-gates/u07-root-history/replay/root-history.json
```
