# 上坡起跳与当帧边缘落地

固定25 ms 的正式 Playground 第六组从实际踏步前态接受 Jump，首扫以正竖直速度撞上原 Stairs_01。相同原生 capsule、网格、位置、扫掠方向和长度得到2.61746 cm阻挡。UE `PullBackHit` 后横向接触位于完整胶囊边缘内，满足 `ShouldCheckForValidLandingSpot`。

源停止点的84 cm短胶囊命中台阶顶，法线向上，命中点低于完整胶囊下半球。因横距大于 ALS 15 cm停靠半径，原生15 cm窄扫也复算通过。两段合计高度低于42.6 cm，Walking 余时有效。纯竖直和更高起点控制均扫空。数据、请求和源码身份见 `identity.json`。

正式验收只在同一 Jump 产生有序 Grounded→Air→Grounded 时重试输入，共用原5秒计时，仍要求18组真实 Air、较高落地和原动画恢复。原25 ms报告完成5组。恢复后18组通过，记录第6、12、18组的同帧落地。默认可变帧步亦通过18组。运行证据在 `../bevy-0.20/uphill-*`。

```sh
d=docs/audit/bf144/u07/uphill-launch-window
for case in first-sweep source-floor perch; do
    pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALSExtras/Environment/Meshes/SM_Als_Stairs_01 -GeometryQueryProbe "$d/$case.request.json" -Output "target/$case.native.json"
done
cargo test --profile play --lib --jobs 2 -- manual_playground_integration::uphill::tests::
pwsh -NoProfile -File tools/run_animation_visual_validation.ps1 -Case uphill_jump -FrameDeltaSeconds 0.025 -CaptureCount 0 -FunctionalOnly
```
