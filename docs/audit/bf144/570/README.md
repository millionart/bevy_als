# BF144.570 普通落地向模拟台施力

在相同高度和速度下垂直落到台面中心。原生比较正常动态台、关闭 CMC 物理交互的动态台及静态台，三个场景的站立下压力均设为零。Playground 比较动态台和静态台，记录实际落地事件、基座、台体质量及落地前后速度，并核对台面与胶囊之间的物理接触冲量。

三个原生场景的落地命中逐字段相同。开启交互的 100 kg 动态台获得 −0.049983 m/s 竖直速度，另两组为零。Playground 两组均从 Air 切到 Grounded 并绑定台面。动态台无物理接触冲量，落地帧和下一帧均维持零速度。

本项核对 D38 共用的碰撞施力入口，消费阶段为普通 Falling→Walking 落地。D37 的站立下压力独立。本项只审计运动组件向支撑物施加的力。

首轮原生包继承的初始化只接受 Walking 输入，因此读数在 `unarmed.native.json.gz` 中标为无落地资格。最终包接回既有 Falling 初始化，验证脚本要求首帧初速与落地委托都实际成立。

封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/570/run_python.ps1 -Name verify`。重采时先备好空输出目录。原生依次运行 `prepare_request`、`prepare_reader`、`build.ps1 -Label impact`、`native.ps1 -Label impact -Request impact`。目标依次运行 `observer`、`compile.ps1 -Label impact`，再运行 `playground.ps1` 的 `dynamic`、`static` 及对应的 `functional.ps1`。截图 0，性能验收暂停。
