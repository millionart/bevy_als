# BF144.446g 冷启动actual force到位置

在原.e Playground增加三个只读阶段：after Motors、完整PreSubstep后/Substep前、Physics.Last后/AlsActualVelocitySet前。按实际App FrameCount关联319冷启动+ready查询，保留实际sensor/config/tracker、TnuaToggle/RigidBody、Sleeping/Disabled、motor、初始solver与积分系数/精确ns时钟，不给缺solver补0。

前19个动画查询处于原入口的SenseOnly/GravityScale0：motor已产生spring，但backend跳过应用，不能只看motor便声称发生移动。其中一帧Sleeping且无SolverBody，实际位置/速度保持，单列未积分合同。App frame24（query19）切Enabled/GravityScale1才开始抬升；此开关为原manual入口的稳定sensor验收流程，未修改它。

319力系数/完整XYZ raw积分逐值复算，79个cold spring输入逐值来自实际float_height、sensor proximity、strength400及tracker gravity。cold raw位置全等最终位置，Y速度在末阶段清零；ready240的raw/final逐值等于已封存.443b并沿用其完整floor消费者闭环。

首抬升offset0.0040000081062316895m、net acceleration1.6000032424926758m/s²、increment0.008888907730579376m/s，六子步后raw Y0.9210370182991028/raw VY0.05333344638347626，最终VY0。源实际floor distance1.9999909400939941cm在有效带，位置保持0.9199999570846558。保持该帧全部actual输入仅去spring因子的独立核恢复源位置；去此因子控制反例退出101。这里扩已有D31当前冷启动consumer，不新编号、不吞并所有318轨迹差，也未修改query或阈值。

初版缺TnuaToggle/RigidBody资格及“一律awake有SolverBody”假设被实际数据纠正，有限旧baseline及失败报告保留。删除三个观察数组使结构合同失败；源码SHA恢复同target六库重编/原场景复跑后，完整数组与模型/分析精确恢复，公共animation functional-only通过。

全部长期原始数据、源事实、脚本/负例、消融/恢复和manifest在Git本目录或明确引用的原Gate目录。截图0、性能暂停，GPU周边不验收。复验用 `pwsh docs/audit/bf144/446g/run_replay.ps1 -Label restored -Build` 与 `pwsh docs/audit/bf144/446g/run_python.ps1 -Name analyze -Case restored`。新runtime使用新Label，不覆盖封存目录；整体Root继续开。
