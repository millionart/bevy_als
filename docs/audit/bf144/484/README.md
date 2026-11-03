# BF144.484 D276 足部反馈

沿 458 原 Playground 场景重新观察 319 帧。新增字段记录 FootIk 更新后、当前动画求值前的 IK 目标链、armature、pelvis、锁定状态和 RigInput。剥离这些字段后，完整旧历史逐字段相同。原场景功能校验通过。318 个有效帧均选择两侧 IK 骨，处于站立接地域，未采到动态基座或传送。初始化的一个无效帧单列。

2,544 个链内已发布骨变换等于前帧 Root。目标选择、世界矩阵和最终锁定状态经生产 FootIkApplyPlugin 注册后在隔离夹具中重放，318 帧完整精确。基线的 1,272 个足位置和旋转字段与真实当前帧 VM 读取的 RigInput 一致。夹具使用原骨树、采样的外部父级和实际锁定设置。

普通帧先用上一帧 armature 建立生命周期，再恢复实测锁定前态。去掉 armature 初始化会使第 146、152、163、246、252、263 帧的状态与 RigInput 失配。每个 D276 对照只换同一前帧发布时的旋转，其他输入和上一帧锁定状态固定。该对照按单帧独立求值。

636 侧中，149 个目标旋转、105 个最终旋转和 103 个写给 RigInput 的旋转改变。105 个最终旋转变化中，85 个未锁定，20 个部分锁定。位置、锁定量、锁定位置和锁定旋转的同输入对照相等。首个 RigInput 差异在 query100 左脚，沿目标链只替换 prior Root99 发布的 ik_foot_l。锁定量为 0.6666666269302368。RigInput 旋转 x 从 0.08631793409585953 变为 0.08631795644760132。

移除 D276 对照后，最小帧 RigInput 差异消失。用当前 Root 替换前帧时，目标矩阵、pelvis、锁定状态和 RigInput 同时失配。三个消融均失败。源码和输入逐字节恢复后，同 target 重编的完整结果字节一致。源 GetSocketTransform 和组件骨空间阶段已读，下一项用原生数据校准该段，并沿本项验证的 RigInput 差异进入后续 VM。

观测夹具初版尝试访问私有 profile，并借用了临时骨路径。改为使用生产插件嵌入的 profile，观测存储拥有所有权的骨名。重放初版的读取借用冲突改为复制值类型组件。编译失败报告随证据保留。

从项目根运行 compile.ps1、run.ps1 -Label observed 和 functional.ps1 -Label observed 可重取场景。prepare.py 可读取压缩归档。独立重放用 run_python.ps1 -Name ablate -Case restore 恢复源码和输入，然后运行 run_replay.ps1 -Label restored -Build。normalization、current-root 是单帧消融。no-prime 使用 replay.no-prime.rs 与完整输入。facts 和 identity 需要本地 UE 与 Bevy 源根环境变量。长期记录在 Git 目录。截图关闭，性能验收暂停。
