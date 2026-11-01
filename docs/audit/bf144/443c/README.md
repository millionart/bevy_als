# BF144.443c：胶囊floor尺寸的scale来源

Source CapsuleComponent.GetScaledCapsuleSize读取ComponentTransform保存的Scale3D成员，radius用source XY较小轴、half用Z；GetScale3D不从rotation矩阵提取。Native .443 source240 root scale全部1，Character的root为Capsule。Target通过TransformHelper得到GlobalTransform affine，再分解scale用于同一movement floor半径/半高。

原Playground240实际local scale和Collider.scale全部1、无parent。用相同local TQS经公开GlobalTransform核，240实际helper scale逐值精确；170帧得到非1的scale/radius。固定同一base半径/半高、位置、shape、filter和其它算法，仅换为保存scale再执行public floor query，45帧最终Y不同。29号base radius0.29999998211860657、actual radius0.2999999523162842；current Y0.9215173125267029，source stored-scale-only控制Y0.9215172529220581。

相同rounded quaternion以double矩阵求值，162帧projected scale仍不是保存值1，29号double scale为0.999999924083854。故差异不只是矩阵算术精度；source保存字段与target重新分解是不同合同。D232是多pose提前float求值，D151是速度局部投影，D11是明确非均匀物理shape，D165是单位舍入；本项锁同base float尺寸和unit-root输入，登记D274/P3，不声称整个source世界反馈恢复。

移除affine scale消费而用保存scale，实际radius反例应失败退出101；删除新增scale_inputs观察，结构合同拒绝。三个源码文件由Git内*.final按SHA恢复、同target库重编后，完整replay JSON byte一致、完整240 runtime记录精确恢复。公共animation functional-only核baseline/restored；GPU pass延续用户周边排除，不改场景/组件/生产renderer，截图0，性能暂停。

生成器首版错选原main早期ManualDuration块而编译失败，改为唯一app.run前最后注入；错误与修正保留，没有修改生产。不能把cached GlobalTransform碰巧为1当作source scale证据，不能把root资格泛化父级、nonuniform或shear场景。

```sh
python3 docs/audit/bf144/443c/check.py self-test
pwsh -NoProfile -File docs/audit/bf144/443c/run_matrix.ps1 -Label replay-new-build -Build
pwsh -NoProfile -File docs/audit/bf144/443c/run_matrix.ps1 -Label replay-new
pwsh -NoProfile -File docs/audit/bf144/443c/run_python.ps1 -Name finalize
```

输入、完整source/target getter摘录、runtime/model/control、失败和消融恢复、六库及396身份、哈希清单均在本Git目录，临时二进制只作可重建缓存。实际source原始uasset不变，累计271项差异（116P2/155P3），生产未修。
