# 相机球体扫掠

本地 `CS_Als_Default` 的半径为15 cm。原生 `SweepSphereVsBox` 与 `SweepCapsuleVsBox` 调用同一 `LocalSweep`。两处同输入命中距离为5.00679 µm和75 cm，原目标为29.9774 µm和75.00138 cm。复用已有源GJK的点支持后得到4.97818 µm和75 cm。

原生零距离满足 `HadInitialOverlap`。半径加1 cm后乘原mesh scale，得到约0.83333 cm MTD，应用后重扫回到原接触点。旧测试把Parry的正时误差作为接触期望。源端正时、零时、真实穿透和扫空的原始结果均保留。

只改回原相机调用，三项定向失败。恢复源码字节后相机、胶囊共33项通过。正式 Camera 的11项及渲染功能通过，报告在 `../bevy-0.20/camera-sphere-round-1/`。

```sh
cargo test --profile play --lib --jobs 2 -- als_camera:: source_capsule_sweep::tests::
pwsh -NoProfile -File tools/run_animation_visual_validation.ps1 -Case camera -CaptureCount 0 -FunctionalOnly
```
