# BF144.449b glTF单位系数与实际Root首高度

fresh Stand/time0/correct SKM的raw与compressed两模式pelvis Z均96.75060272216797cm。ALS在源厘米FTransform中消费该值；依现行中性物理单位映射投影F32应.9675060510635376m。UE glTF exporter先cast FVector3f，乘float ExportUniformScale默认.01f（实际.009999999776482582），生成.9675059914588928；当前原文件、loaded clip/public sampler与实际Idle叶全部精确等后值。该字段不受raw/compressed模式差影响。

只在诊断内存克隆原Stand GraphClip，保原duration/skeleton/其它曲线，唯一pelvis translation Y换为物理单位投影。公开核完整Pose断言只有这一字段改变；仅Idle evaluator路由clone，Head参考继续原handle。原Playground239 actual叶、前98个正步长BaseProvider Y及首最终Root Y恢复源值，所有其它动画输入/clock、motor与后置相机逐值保持。跨a/b程序的rows仅camera本地Entity ID整体+1，原始值保留；断言每次只有一个稳定camera引用并按角色关系比较，其余字段精确，不作数值容差或物理差处理。

删除唯一handle路由时，保留clone与数据构造，全部98输入与首Root返回原值；移除once guard在第二次转换的原始高度前置断言失败。四个坏路由/高度/重复应用/变输入反例拒。三源码原始bytes/SHA恢复、同target六库重编后，全部controlled新旧数组及check精确恢复，两functional Gate通过。缺宏及第18个SystemParam编译失败在runtime前修正为显式导入/Res tuple与排序，没有运行失败二进制。

确认D275/P3，累计272（116P2/156P3）。D224原平台数据因素不含这个raw==compressed的单位参数首差；D165是Rig厘米状态计算，D232是多pose求值前的数值空间投影。本项为单叶导入单位系数，未混合即可复现。旧.423整份prepared数据替换同时覆盖多个数据因子，未分离此系数，有限复开仅修正这一个位置输入归属后收口；其它44T/31Q记录与未知Root因素保留，不一概重分配。

原glTF/bin、reader、生产、引擎和转换器未修改；当前只有诊断内存控制和Git审计记录。参数原始历史无损gzip/raw SHA，失败/恢复/身份与manifest全部Git管理。复验使用已有输入 `pwsh docs/audit/bf144/449b/run_python.ps1 -Name check -Case restored`。无可见故障或全Root恢复声明，截图0、性能暂停，GPU周边排除。
