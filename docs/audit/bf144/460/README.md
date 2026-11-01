# BF144.460：真实射线几何资格与源连续 Rig 输入

沿459已校准的真实319次Rig输入和636次射线，本项使用同六个冻结库的公开`RigQueryContext`，从原Playground manifest中取两块Floor_20x20的实际位置、尺寸和局部中心。全部636个命中坐标/normal精确复现；最初假设30×1×30单Box有266次不符，已拒绝。fresh仓库reader重新读取原Floor资产，实际ExtendedBounds、BodySetup Box和碰撞网格均证实中心(0,0,-25)cm、半尺寸(1000,1000,25)cm，不由射线结果拟合参数。

负控制将整个世界下移1m，636次全变；物理删除局部中心−0.25m偏移后，同target重编运行，636次全部失配。按源码字节恢复、同六库target重编后，完整636行JSON与原通过报告逐字节相同。失败报告、原假设fixture、被消融源码和恢复源码都保留。

既有原生RigNodeHistory入口创建两份独立连续319帧实例；每帧输入完整实际79骨Pose/曲线、11个RigInput字段、消费时钟和MeshToWorld，分别附左右原实际射线作reference query。638帧的before Pose/曲线、外部字段7018项、请求和实际Forwards Solve world均精确；初始化delta0，随后连续工作状态，没有注入源输出/working memory。两源实例输出Pose/曲线全相同，319帧曲线也等于当前输入。完整25201骨对比投影后的目标输出，T/Q/S仍有1324/1845/0个骨行差。

源入口只支持单Box，本次取两原地板的几何并集；它与目标两Box的物理表示不同。636条原实际射线的源reference query中，288个命中点投影后不等于当前目标，normal相同。首例源投影高度约7.1e−17，目标−2.98e−8；不放宽阈值、不覆盖命中。reference query在Forwards Solve后独立发出，不能冒作源足部unit内部实际请求/命中。因此本项只完成输入和查询前置资格及输出清单，不能把全部姿态余差归给VM算法、D242或某个精度项，也不声明完整Rig/Root/ALS等价。下一步需要分离实际内部查询与共同输入算子。

4个坏数据fixture分别破坏delta/world/Pose/RigInput，都由实际检查器拒绝。395个其余身份、457 reader身份和六库不变；生产/reader/转换器/源资产/引擎均未改。新确认差异0，累计272项未修。未运行新角色场景；这里是从原真实入口数据派生的focused诊断，不替代Playground验收。截图0，性能验收暂停。

复验从项目根目录执行：

- `pwsh -NoProfile -File docs/audit/bf144/460/run_replay.ps1 -Label <new> -Build`：公开target查询资格。
- `pwsh -NoProfile -File docs/audit/bf144/460/run_python.ps1 -Name prepare`后`run_native.ps1`：重建请求并读取源连续Rig；`run_floor.ps1`重读原地板。原生实例ID允许变化，输入/姿态数值合同不变。
- `run_python.ps1 -Name check_native`、`-Name inventory`、`-Name controls`、`-Name finalize`：可直接使用已压缩报告。`facts`/`identity`需要本机配置及`ALS_UE_SOURCE_ROOT`。
- 消融时用`replay.no-local-center.rs`覆盖`replay.rs`、同target重编，预期退出101；恢复`replay.rs.final`并重编，必须重新通过。正式源码保持恢复状态。

长期证据均位于本Git目录及summary列出的已跟踪外部证据。331MB原生报告无损gzip，压缩映射验证原始长度/SHA和解压字节；binary与普通私有日志可以重建，不作为唯一证据。seal检查每份文件已Git跟踪、压缩证据哈希和解压后的本地路径隐私。
