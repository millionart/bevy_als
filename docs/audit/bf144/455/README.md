# BF144.455：Head 到 Root 的本端增量库存

对同一318帧，在每一端分别比较RigSource输入与Root输出的原79骨。它们之间包含ControlRig和Ragdolling；当前语料没有独立ControlRig输出和目标VM实际状态，不能将所有变化直接归Rig，也不能凭跨引擎差表相同断言没有写回。

| 数值域 | 位置变化骨行 | 旋转变化骨行 | 缩放变化骨行 |
|---|---:|---:|---:|
| source原始F64 | 1902 | 1921 | 992 |
| source投runtime F32 | 1268 | 1902 | 788 |
| current实际F32 | 1572 | 5184 | 819 |

每域25,122骨行；每端曲线的键和值在这段链路内均保持。source实际RigInput两项UseIK均318次True，FootTransformsValid首帧False、后317帧True，三bool全部精确getter。原记录未捕获LocomotionAction，不能据此排除Ragdolling活动。source与target分开保存，不用相同骨名或零变化推断相同writer。

fresh原生main AnimBP通过仓库reader读取：Head→ControlRig→Ragdolling→Root三条边与当前生成图一致，Alpha1、pose/curve传递开启、全局空间传递关闭；当前图与实际语料提交时字节不变。源码记录输入pose从当帧开始、按definition条件重建、曲线传递与全部骨写回。它证明入口与静态边界，尚未证明本域VM输入、writer执行和独立Rig输出。

移除F32投影后丢失原始/投影差异边界，合同失败；源码字节恢复、同解释器重跑后完整分析与表格字节精确。错帧、错delta、缺Root、错骨和错误坐标基五负例拒。最初动作读取错把RigInput getter当主状态，保留初版结果，正式结果改为已捕获的实际Rig bool并明确标记缺动作字段；负例只复制被修改的首帧，删除无用整包深拷贝。

复验：`run_python.ps1 -Name inventory -Case <new-label>`；投影消融使用`ablate.py remove/restore`后同runner重跑。`facts`/`identity`要求环境变量`ALS_UE_SOURCE_ROOT`。主图native报告和四大表无损gzip，原字节SHA、Git跟踪及外部依赖由manifest/summary绑定；原请求与完整原始语料均在Git管理目录。

下一必要读点是当前真实ControlRig输出cache、runtime层级与external/working值、输入绑定，保留下游Ragdolling独立边界；不拿旧251步或临时空场景替代当前318域。没有新D，272项未修；生产/reader/引擎/原资产不改，截图0、性能暂停，GPU周边排除，完整Rig/Root/ALS等价保持开放。
