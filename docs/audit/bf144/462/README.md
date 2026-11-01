# BF144.462：源 VM 空间参考射线原生能力

461已经证明实际636次FootOffsetTrace的参数在调用后没有覆写，但旧RigNodeHistory只接受世界空间reference ray，无法提供源ToWorld/ToVM和同参数查询的原生读数。本项仅给原frame协议增加严格bool `reference_ray_vm_space`，true必须同时提供mesh_to_world。省略/false沿用旧世界空间语义；新增4个结果字段只在true时输出。

源FootOffsetTrace.cpp使用实际ExecuteContext.ToWorldSpace构造世界起终点，查询后用ToVMSpace及context transform的InverseTransformVector读取局部命中/法线。新能力直接调用相同UE公开API，使用已有同一静态Box world、Visibility complex模式和Owner过滤，在Forwards Solve之后独立重发参数已由lifetime证明的参考射线。它是同参数API资格，不是unit内部hook，也没有把目标hit送入源。

两组连续319帧使用原460的实际Mesh/79骨输入/变量/world/时钟，左右参考局部ray分别来自461实际source work参数；共有636条真实ray，初帧复用首有效局部ray额外2条。旧报告按缺字段合同先失败。新读数636份VM hit的Z转源厘米float、VM normal分别逐值精确实际源unit的OffsetLocationZ/OffsetNormal；这里实际normal为Up，坡度修正恰0。638份完整before/after Pose、externals、执行事件/顺序、时钟和工作数值与旧记录精确。

原始完整工作diff有156条路径，全部是FCachedRigElement.ContainerVersion的exported_text。fresh RigHierarchy.GetTopologyVersionHash明确混入this指针，CachedRigElement.UpdateCache将该hash存作实例缓存身份。因此仅该类型中的活动实例hash按语义比较；-1无效状态、Key/Index和所有计算数值保留。每帧必须只有一个活动hash，其他类型文本不处理。自测确认只改活动实例hash可比较，改Index、改成-1或改数值均被拒绝。原始diff和未修改报告仍保留，不把缓存身份差吞为普通数值容差。

必要性控制：删除两次VM→world转换后，638条参考查询全由命中变未命中，636真实ray的资格全部丢失；全部Pose、externals、world、时钟、事件和visited保持原样。删除mesh_to_world父条件后，原本非法的缺world请求获得成功。恢复源码bytes、完成同package构建终态并核project/package SHA一致后，完整636条world endpoints/world hit/VM hit/VM normal表与基线逐字节相同；最终类型错误和缺world都返回7。

兼容性要求整个旧Rig history probe（仅按上述源定义比较缓存实例hash）、旧default character probe和三轴CurveVector完整probe精确。未改source资产、源ALS/UE或Bevy引擎、生产、转换器/图。Reader从457身份显式迁移，其余395身份和六个冻结库不变。

复验从项目根目录：

- `run_python.ps1 -Name prepare`从460原请求和461实际source参数生成新请求及严格坏请求。
- `run.ps1 -Label <new>`原生重读；`run_python.ps1 -Name check -Case <new>`核完整源状态与636项输出合同。所有脚本均在本目录。
- `run_python.ps1 -Name ablate -Case no-vm-transform`、`build.ps1 -Label <new>`、`run.ps1 -Label no-vm-transform`；完整check必须失败，ablation_check确认全部638次丢失命中及Rig输出不变。
- `ablate -Case no-parent-guard`后构建，`run.ps1 -Label no-parent-guard -Request bad-parent -ExpectedExit 0`必须接受；按`ablate -Case restore`恢复并完成构建终态，再运行restored与两个坏请求，分别期待0/7/7。
- `run.ps1 -Label default -Request default`、character-default.ps1、curves.ps1及`run_python.ps1 -Name compatibility`验证完整旧域。
- `run_python.ps1 -Name cache_selftest`验证缓存比较边界；facts/identity需要本机配置及ALS_UE_SOURCE_ROOT，finalize重验全部已存证据。

自动流程复用有focused自测的固定wall/no-progress watchdog；机器报告记录进展源、等待和退出原因。长期报告/请求/脚本/源码bytes/消融与恢复结果全存Git目录；大原生报告无损gzip并核原SHA，普通含本地路径的日志仅作临时输出，必要错误经去路径后持久化。没有target目录中的唯一长期证据。

下一步用已资格化的源局部输入、原生context world ray/hit/VM结果，分离目标查询精度和厘米float offset阶段，并继续完整连续Rig/Root差分。该能力不等于全Rig通过。无新D，累计272（116P2/156P3）未修；截图0，性能验收暂停。
