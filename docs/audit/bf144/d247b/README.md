# BF144-D247b 两骨 IK 直接向量旋转

UE `SolveBasicTwoBoneIK` 用 `TransformVectorNoScale` 旋转每骨 primary/secondary 轴；`TwoBoneIKSimple` 的 PoleVectorSpace 依据 Direction 或 Location 分别旋转，后者再平移。目标原先在这两个公开路径直接使用 glam 四元数乘向量，现复用 D247a 的源 cross/FMA 算子，保留 Pole 类型和位置规则。

fresh `ik.request.json` 从 `.479` 的 5192 个请求中选择源默认/primary-only 和 current 数值域默认/primary-only 各两条。四份原生 root/joint quaternion 与封存记录逐值一致。共同数值右腿默认控制的 native thigh w 投 F32 为 `−0.21039418876171112`，旧目标控制为 `−0.21039420366287231`；其中 current 米制数字用作源 API 的同数字输入，仅隔离算术，不称为实际源厘米历史。公开求解器 fixture 已使用该控制，是否通过仍待统一验证。

请求、完整原生输出、源事实、哈希和选择身份在本目录，压缩数据已逐字节核对解压；`.479` 的完整输入、五因素隔离及消融恢复仍作为历史证据。额外 F32 诊断求解器、非 ARM 的源 FMA 配置、其他 IK 算子和完整姿态不在本次原生证明范围。focused 测试、整批回归、消融恢复、图审计和公共 Playground Gate 待全部静态修复及 Bevy/依赖迁移后执行。
