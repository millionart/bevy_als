# BF144-D242 Control Rig 初始全局变换

本次重新读取 `CR_Als`，蓝图层级与编译 CDO 的 79 骨初始值逐项一致；`AB_Als` 主图的 Control Rig 节点 `bSetRefPoseFromSkeleton=False`。本地 UE 源码从 CDO 复制层级，ALS 足旋转首次读取 calf/foot 的初始全局旋转，TwoBoneIK 未提供骨长时读取相同层级中的位置与 scale。两份原生读取、请求、源码片段、哈希和完整变化清单均在本目录，原生压缩结果已逐字节核对。

`tools/import/update_rig_initial_globals.py` 复用现有 Control Rig contract、坐标转换及 RON 渲染器；在写入前要求蓝图/CDO 骨数据一致、生成定义除初始全局值外与现有定义一致，并要求运行图中原定义恰好出现一次。它更新目标 `CR_Als.program.json` 和主图内嵌定义；此次仅 71 骨的 365 个分量变化，初始局部变换、拓扑、程序和其他参数均保持。原始 UE 资产与原始导出未改。复跑命令：

```sh
python3 tools/import/update_rig_initial_globals.py --source docs/audit/bf144/d242/rig.native.json.gz --program assets/ALS/Character/CR_Als.program.json --graph assets/ALS/Character/AB_Als/AnimGraph.animgraph.ron
```

已写 `initial_globals_match_native_cdo_hierarchy`，从原生 79 骨记录独立复算轴与厘米转换并检查公开 hierarchy getter；已写 `loaded_control_rig_uses_generated_initial_globals`，比较真实加载图与生成定义。两项尚未运行；关联图的完整转换对比、受控脚旋转/骨长消融和 Playground Gate 均按用户顺序在全部静态修复及 Bevy/依赖升级后执行。D163 算序、D165 单位舍入和其他姿态反馈边界不因本项数据更新而视为已修。
