# BF148.02 急停与 Slot 发布

`AlsAnimationInstance.cpp` 1575–1628 以实例自己的运动输入、朝向和姿势选择速率与过渡序列，1780–1794 将该序列作为 Transition 动态 Montage 播放。`AlsCharacter_Actions.cpp` 90–113 由当前角色自己的动画实例播放 Roll Montage。根图原生记录确认 Transition / Grounded 和 PostLocomotion / Locomotion 两个原生 Slot 各有真实的 Source 和 Pose 连线。Standing 编译记录包括 4 个状态机、40 个资产播放器及 Stop 的实际子图。可用 [native.json](native.json) 的资产和参数复跑 `tools/run_ue_source_reader.ps1`，解压报告核对 SHA-256。

生产根图消费 `transition.pose` 和 `post_locomotion.pose`。在 `src`、`tools` 和现有 RON 核对同类键。旧 `stop_*` 只有 Validation 图读到，仓库无注册那些图的活动入口。移除全播放器广播及专用状态。旧 Transition/PostLocomotion 的 Pose、layer weight 和曲线别名无人读取，旧 PostLocomotion weight 也无人读取。Validation 图读取的 Transition 激活权重仍用原键。Roll/Get Up 共享已经发布的三个 PostLocomotion 常量。`roll.pose` 另有运行报告和实际采样测试消费者。

`AnimNode_Slot.cpp` 39、55、58、119 按 `SlotName` 注册、取权重并求值。源节点 GUID 分别为 `99688c67-43aa-bd26-2097-3d8fbfb6019a` 和 `ccaab6a2-424d-b00d-6794-f3a705d6f8c2`。`f67952b3` 引入手工根图，`f10c3594` 将旧 Pose 键接到该根图的转发节点，`7dcff912` 删除这两个消费者。当前生成根图用同源 GUID 的 SourceSlotNode 消费实际 Montage 姿态及上下文，原生读数与当前文档没有声明旧 Pose 键的扩展合同。

`StartStopState` 在脚部 pin 与公共报告被读取。两个 `AlsCharacter` 会让原 `single_mut` 提前返回。最小夹具固定一个应进入 Started 的角色和一个应进入 Stopping 的角色，原实现第一个角色仍是 Idle；改为循环各自推进状态与方向后复验。同批验证实际 Quick Stop 通知、Transition 采样、Roll/Get Up 与脚部 pin。

[作者性清单](../../../../tools/animgraph_authoring/bf148_runtime_slots.authoring.json) 绑定原生读数、实际根图和公共 `standing_stop`。真实入口先跑 startup，再覆盖 Standing Stop、Grounded Roll 和 Get Up。零截图，用功能验收范围。

第一轮公开 Standing Stop 在两个版本中都出现同样的观察合同错误。四种 Stop 成功，三个 Quick Stop 的最早失败均为 `standing_states_fsm: static`，手动覆盖 10/10、根姿态运动和图错误检查均通过。前后对照见 [standing-stop-comparison.json](standing-stop-comparison.json)，原始报告校验和见 [failed-runtime.json](failed-runtime.json)。原生 Standing Idle 的 A_Als_Stand_Pose 读取空接的 ExplicitFrame=0，Quick Stop 动画由原生 Transition Slot 求值，因此观察器在 Quick Stop 中直接读取同源 Slot 的 Pose。验收保留普通 Stop 的运动链，以及原有选片、时钟、方向、最终姿态和生命期断言。
