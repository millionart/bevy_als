# BF144-D244 注册姿态先于角色物理

fresh 非零帧原生角色记录与 D243 共用：`docs/audit/bf144/d243/first-tick.native.json.gz` 的注册快照发生在 world 0，动画输入为 Grounded、pending，PoseGrounded、PoseStanding 和左右脚 Ik/Lock 六条曲线均为 1；首个正常 world tick 才进入 Fall。UE 源码先注册 mesh 并以零 delta 更新/刷新，ALS 动画实例复制角色当时的 locomotion 等状态。源码摘录、原生请求、原文及压缩哈希均可通过 `source-facts.json` 与 `native-identities.json` 复验。

目标角色的动画场景异步装配，而原角色物理体可在场景就绪前运动。出生阶段暂不让该角色物理体参与积分或让 Tnua 更新，项目自身的运动、支撑选择和状态事务也等待；场景的实际根图、骨架和当前 overlay 资源就绪后，原有输入生产链读取真实出生状态，以零 delta 求值一次并生成 pose 与 curve，再放行角色；下一个普通帧按 D252 选择出生支撑并执行首个物理步，D243 的普通图更新也在该帧消费 delta。全局物理调度继续服务其他角色：保证的是该角色的首个物理步发生在其出生 pose 之后。

`root_and_linked_sequences_consume_the_player_clock` 已补未就绪不求值、仅零步长初始化和下一次正常时钟的控制；`birth_pose_and_curves_precede_first_character_physics` 已写入真实角色、主动延迟场景加载并施加水平初速、出生高度与 Grounded/Running 状态、六条曲线及首个物理步的观察。两项均尚未执行。零求值后 pending 清理寿命仍归 D149；RigidBodyDisabled 仍允许碰撞检测和空间查询；加载期间其他物体的互动、加载失败和异常图的公共 runner 报告、场景重建/其他外部调用方式以及调整的各个等待分支仍需最终 Gate 与消融确认。旧 `.252` target 原始报告缺失；图审计、测试、消融和 Playground 验证均待全部静态修复与 Bevy/依赖升级后运行。
