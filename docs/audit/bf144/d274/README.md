# BF144-D274 根胶囊 floor 缩放来源

`source-facts.json` 固定 fresh UE 角色根胶囊、保存的 `Scale3D`、Walking 到 floor 查询的调用链、目标两处调用及 `.443c` 第29帧的原始对照。完整240帧与45帧下游影响复用 Git 跟踪的 `.443c` 记录和哈希，不把源控制称为整条世界轨迹恢复。

无父级角色的行走事务及物理后 floor 重查共用 `Transform.scale`；有父级时沿用原仿射分解。蹲伏、Mantle 和其他 scale 消费者未列入本次已证范围。已写第29帧精确半径反例、非均匀根缩放/父级控制，以及实际 floor 同步用例。

全部条目实现后运行 focused 测试和原 Playground 运行 Gate。消融时让无父级路径重新读取仿射 scale，预期第29帧半径用例失败；恢复源码字节并在同一 target 重编后复验。以上测试及消融尚未运行。
