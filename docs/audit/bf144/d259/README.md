# BF144-D259 足锁定 twist 的 Atan2

fresh UE 源码表明 `FQuat4f::GetTwistAngle` 调用 `FGenericPlatformMath::Atan2(float)`，后者使用七项 float minimax、零输入早退及象限恢复；本项仅将足锁定 twist 的系统 `atan2` 替换为该算子。源弧度折返和 D260 的 float 角度系数保持各自阶段，其他 Atan2 调用不在本项范围。

旧 `target/bf144/420` 的四份报告当前缺失。`source-facts.json` 引用 D262a fresh 原生请求、结果和 reader 报告，保存实际前态和受控 80° 的 FQuat4f 相对旋转及原生 twist。已写两组原生值、零输入和受控限位分支用例；不以缺失的旧结果替代原始证据。

这些 focused 用例、将算子换回系统 `atan2` 的消融、源码字节恢复后在启动器同 target 重编、完整图静态审计及公共视觉 Gate 均待全部静态修复和 Bevy/依赖迁移后执行，目前尚未运行。
