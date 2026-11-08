# BF144-D09 地面射线回退高度保护

fresh 诊断 reader 使用源角色、地面和 45° 侧墙：出生高度 100 cm 的穿透控制在首帧得到 sweep FloorDist −30 cm、line LineDist 10 cm，源角色横向离墙但保持 100 cm；无墙相同高度控制调整至约 92.15 cm。92.15 cm 的有墙/无墙控制均保留高度。UE `AdjustFloorHeight` 对 line fallback 且 sweep 小于最小距离、line 不小于最小距离直接返回；ALS source 保留穿透 sweep 信息以供后续独立脱离。

目标现有 floor 结果已保存 sweep 和 line 两个距离；本项在 `adjust_walking_floor_height` 与公开角色状态同步的高度调整处复用这一源条件，不跳过侧向穿透恢复。已写严格阈值测试及加载角色的高位墙侧实际查询、两处高度消费和横向脱离控制，均未运行。

首次诊断请求漏配该历史 reader 必需的落地点，reader 对空 `LandingDeck` 调用造成 SIGSEGV；watchdog 按无进展上限结束。失败请求、报告和脱敏栈保留；第二次仅在远处增加必需落地点后成功提取，reader 的源码/二进制身份与 `.572` 封存一致。所有原生请求、输出、源码、哈希及 reader 重建入口见本目录，压缩结果逐字节核对。旧 `target/bf144/04/line_final/` 目标报告缺失；测试、消融、图审计和真实 Playground Gate 待全部静态修复及 Bevy/依赖升级后运行。
