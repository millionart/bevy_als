# 两倍体型布娃娃起身

正常 `ragdoll` 场景从高处放下两倍角色，等待布娃娃地面跟随稳定后退出。原版 ALS `StopRagdollingImplementation` 以 `TransformPositionNoScale(GetBaseTranslationOffset())` 恢复 mesh，再以 KeepWorld 挂回胶囊。原生实际两倍角色起身期间，胶囊中心高约 181.9cm、mesh 原点高约 89.9cm。单位倍率控制的 mesh 原点约 −0.1cm。两者均在 Walking 启动蒙太奇。`source-first` 还保留单位倍率在较早同一时点退出的 Air/无蒙太奇控制，以及两倍低跌落控制。

旧目标进入起身后短暂发布 Air，虽仍有可走地板。阻挡输入时 Tnua 的 displacement 暂空，后置模式同步把这当作丢地。起身中的 Walking 现在服从当帧实际可走地板。移除地板仍进入 Air，空中退出仍不启动起身。

旧目标的 Feet 和 Control Rig 查询用 avatar 挂载点，实际动画播放器已经按原版偏移挂回，在该场景高出 0.92m。源 `RefreshFeetOnGameThread` 从同一 skeletal mesh 取目标并换算组件位置，Rig 也用该组件。统一实际 mesh 坐标后，旧场景在 0.98s 约 156° 的右大腿和右脚旋转误差消失。0.54–0.86s 采样的六个腿部相对旋转与原版在 0.01° 内，0.96s 右大腿和右脚分别约 0.014°、0.04°。早期物理快照、尾部混出有独立差异。

`compare.py` 用相同蒙太奇时间配对，去掉角色朝向并比较父子局部旋转。原生请求可将 `reader-observer.patch.gz` 解压应用至项目读取器后，用 `tools/build_ue_source_reader.ps1` 和 `tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/B_Als_Character.B_Als_Character -CharacterAnimationProbe <source.request.json> -Output <output>` 复跑。取证后读取器源码按原哈希恢复并重编。目标 `ragdoll` 报告以原地形和相机、60Hz、截图0采集。
