# BF144-D273 普通步行的有效 floor 距离带

`source-facts.json` 保存 fresh UE Walking/AdjustFloorHeight、目标普通下扫及单步和多子步消费链，并引用 `.443b` 的240帧、`.574` 的三子步和 `.578` 的制动证据。旧审计记录保留修复前结果，不用作当前实现已通过的证据。

当普通、非 step 的向下胶囊 sweep 命中距离位于 `[MIN_FLOOR_DISTANCE, MAX_FLOOR_DISTANCE]` 时，保留目标高度；其他距离、step、penetration 和 line 路径维持原逻辑。单步及多子步调用同一事务。短胶囊的源查询偏差仍归 D125，不声称本项消除整条轨迹余差。

已写公开物理查询下的带内与带外用例；旧测试主要覆盖台阶或最终 clearance，没有检查普通事务是否在有效带内提前上抬。全部条目实现后执行 focused 测试和原 Playground Gate；消融时删除距离带判断，预期带内反例失败，恢复源码字节并在同一 target 重编后复验。以上测试与消融尚未运行。
