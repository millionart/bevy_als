# BF134 移植差异台账

本轮起始版本 `9105cdf`；对标本地 ALS-Refactored 与 UE 5.8 的当次 source。严重级与任务边界见 `CHECKLIST.md` BF134。仅登记有源事实、可达 target consumer 与可重复触发条件的 P2 及以上差异；旧结论不代替 fresh 审计。

当次覆盖 inventory：146 个源 C++/header（ALS 112、ALSCamera 12、ALSEditor 16、ALSExtras 6），181 个 target Rust，15 个 installed AnimBP root、359 个生成依赖文件和 15 份安装 manifest，文件缺失 0。源 revision 为 ALS `b754d6f0f2bb03741d301f8fb88077ebfe561e17`、UE `396c9f059903aed5fec78ecd3d437a40c6415368`；完整相对路径和 SHA-256 在本地证据 `target/bf134/inventory.json`。B_Als_Character 原生 reader 39.050 秒成功；watchdog self-test 通过。Editor/AI/网络与刻意差异按执行清单排除。

## 覆盖

| 移植闭包 | 本次 source/target 核对 | 尚未核销边界 |
|---|---|---|
| Character/CMC、状态、输入、出生、运动基座 | 实际源码位置与入口清单正在核对 | 算法/配置/坐标/生命周期 |
| Roll/Mantle/Ragdoll/GetUp/Stop 与 montage | 待执行 | 动作门槛、时钟、根运动、退出与重建 |
| AnimBP/FSM、cached/linked Pose、BlendSpace、曲线与同步 | 待执行 | 全递归 source 与当前 runtime consumer |
| Notifies、脚步与贴花 | 待执行 | 消息区间、过滤、所属实例、实际 shader |
| Control Rig、骨架与 IK | 待执行 | 源 RigVM/坐标/当帧 Pose/生命周期 |
| PhysicsAsset、关节、CCD、时钟与回读 | 待执行 | 实际求解器配置与 consumer |
| ALSCamera、相机动画、shake 与 trace | 待执行 | 实例、时钟、坐标、碰撞与 view |
| Overlay、Extras 场景/材质/UI | 待执行 | 资产/装配/输入与显示的源合同 |

## 已确认差异

尚无本轮已确认项。每项记录 source 函数/资产与当次证据、target consumer、触发条件、用户影响、严重级、旧 Gate 漏检原因、最小失败反例、修复、消融、回归与真实入口结果。
