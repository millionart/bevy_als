# BF144.446b 长历史及非零初态原生 fixture

现有CharacterAnimationProbe的每case上限从240扩至实际所需318正步。可选`spawn_xy_cm`为两个有限source厘米坐标，`spawn_yaw_degrees`为有限source角度，缺省均0。沿用原`spawn_height_cm`。原局部测试平面随XY同移，保持既有15m范围的局部floor域；没有修改ALS运动算法、资产或target场景。

请求由已封存target319查询生成：将零初始化单列，其余318普通查询的delta、desired状态、world方向和view heading按已证基映射到source。另有XY(4200,-3100)cm/yaw90°的2步控制，远离旧平面，验证floor平移确实被消费。source Actor初态使用原生TQS与controller yaw读取核对；yaw合同核到float投影，不宣称任意source/target quat位模式相同。

旧reader拒318请求code7，旧2步pose控制被忽略为原点/identity；原始报告保留。扩展后318+2步均实际Walking且floor有效。319超限、错误XY维数、字符串yaw应拒7。消融保留XY、去yaw与floor平移并移除typed guard：初始quat回identity、远处角色无floor/Falling、字符串被接受；三者各有实际读数。源码原始字节/SHA恢复、同native package重编后，新完整character probe精确恢复，旧无新字段240完整probe也必须逐值兼容。

全部原生结果无损gzip，压缩/解压哈希见`compression.json`；请求、工具源码恢复字节、事实、过程/验收结果及身份迁移均在Git。复验为`pwsh docs/audit/bf144/446b/run_check.ps1 -Label restored`。重跑source用新Label，不覆盖封存目录。

本Gate仅证明读取能力和受控源输入可执行；source/target全部初态参数、318自然反馈及Root仍须后继逐项比较。截图0、性能暂停；周边/GPU验收继续排除。
