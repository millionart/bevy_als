# BF144.442：输入方向审计合并

本项是已记录差异的审计闭环，不是修复完成或整体 ALS 等价证明。

| 子项 | 已证范围 |
| --- | --- |
| 442 / .442a | 原 Playground240 的 actual published yaw、实际输入、desired direction 与 Update/Last消费阶段 |
| 442b0 | 可选原生输入数学字段、旧720兼容、两项消融与同包恢复 |
| 442b | 317同条件producer、native clamp/trig/角度阶段、公开consumer；D269/270/271与D266新consumer |
| 442c0 | 实际 controller getter/cache/viewtarget只读字段，旧240整个角色包兼容和恢复 |
| 442c1 | 原场景首次发布、受控移除/恢复；两项消融、同target源码bytes恢复和公共功能Gate |
| 442c2 | 同状态actor/control字段来源反例、独立角度控制/负例/byte恢复；D272 |

最初 ready 样本的侧向小量来自实际 camera yaw，不是固定 raw yaw；该误判已否定。随后 source-first/native 核对登记四个独立差异：float minimax SinCos、额外方向 normalize、角度float表示阶段，以及 manager存在/cache0 时回退错误字段。四者均未修生产。

首次自然无published的两帧只观察到零输入；camera-off受控路径不等同source cache0。source warm cache和无controller手动非零调用未作原生动态验收，完整camera lag、世界反馈/Root与周边内容不在本项结论内。已证静态分支合同和这些动态限制保留于子项，不以父项勾选抹去。

`check.py` 逐份核六个子manifest的原始字节SHA/长度与Git跟踪，并拒绝缺子项、失败Gate及坏哈希。身份迁移依次承接两次只读reader扩展，生产 ALS/转换器/资产和同六运行库未改。没有重新声称已丢失的旧12018全语料得到恢复或复验。

```sh
pwsh -NoProfile -File docs/audit/bf144/442-final/run_python.ps1 -Name check
```

长期证据均在Git目录；临时构建不作为唯一证据。截图0，性能验收继续暂停。整体审计目标保持活动，累计269项已确认差异（116 P2 / 153 P3）。
