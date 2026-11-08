# BF144-D89 外力与冲量的休眠平滑

fresh PA输出与D65a原生记录逐字节相同，`p.Chaos.SmoothedPositionLerpRate=0.3`。源Dynamics更新在积分前分别预置线/角平滑速度：取当前速度、外部加速度乘固定1/30秒、对应冲量速度，再按该系数插值；满足分量近零条件的输入不触发预置，重力在此阶段尚未加入。请求、reader报告、source片段及身份已保存。可执行 `python3 tools/import/import_sleep_forces.py --facts docs/audit/bf144/d89/source-facts.json --settings docs/audit/bf144/d89/native-sleep-force.json --output src/avian_ext/sleep_force_parameters.rs` 重建目标常量。

目标在Avian世界/本地外力累积后、重力预处理前读取线/角加速度并预置源平滑值；现有连接岛采样复用同一源插值系数。项目角色接触冲量通过已有速度接口立即生效，附带记录后续物理帧的冲量，使D88区分显式设速与冲量，并处理两者前后写入；CMC repulsion的同帧设速先于solver body准备与捕获。已写加载rig、连接岛、零力、世界/本地线角加速度、重力排除及接触冲量顺序对照。

测试、消融和Playground均待统一Gate。项目外直接调用Avian冲量API时不携带项目接触冲量记录，其睡眠来源无法从速度组件变更中区分，尚未纳入本项目已识别的调用链。
