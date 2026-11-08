# BF144-D100b3b3c4c2：已有凸包组的源接触与热缓存

对动态 ragdoll box 与静态源 compound 已能唯一识别的目标 hull，真实窄相位先按源整体刚体边界、积分前速度与物理 delta 计算 cull，再尝试已有源几何复用。复用成功时保留旧点、法向和 simplex；失败时用同一 hull 的旧 simplex 运行源 GJK、面/边生成与源活动资格判断，再按源 hull 顺序同步实际 solver 槽。GJK simplex 与几何缓存分开，几何未激活时只要目标碰撞对和形体身份仍存在就保留 simplex。

源生成的两侧局部表面点直接保存为复用初始数据；目标 contact 使用两侧世界点的中点、源法向与 phi，按当前质心和速度更新锚点及法向速度。源和目标生命周期源码见 `source-facts.json`，Arch02 原生边边、Bench 连续帧、源配置和复用证据由 `identities.json` 引用。

已写实际 solver 中 Arch02 单点、普通 body 对照、重复帧 simplex 保留、原生两侧表面点复用及源热重建控制。目标未生成 hull 的发现与新增属于 D100b3b3c4c3；直接 native GJK/配置、测试、消融、全量转换 diff 和 Playground 均待统一 Gate。
