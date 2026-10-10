# 浏览器审阅对照

核对目标：`4e2b6bc447ab47a570c85135eb1b6e0de5aa97c2`。浏览器 continuation_13 的累计记录针对 `9105cdf021f668a86c9d580df2e48b8f57d8233d`，共 33 项；完整 JSON 5999 行已读取、解析且数量核对。UE `396c9f059903aed5fec78ecd3d437a40c6415368` 与 ALS plugin `b754d6f0f2bb03741d301f8fb88077ebfe561e17` 的本地身份相符。

“待原生确认”表示指定提交保留相同生产路径，需当次本地 source 和失败反例后才修复；不是沿用浏览器模型的原生通过声明。后续修复和验证填写同一行。

| 编号 | 差异与当前 consumer | 指定提交判定 | 修复及验证 |
|---|---|---|---|
| R01 | `source_physics_joint::solve_source_joint_velocity_iterations` 未屏蔽非动态端响应惯量 | 确认并已修复 | BF135.06：真实query有限质量Kinematic父`.5!=0`→唯一inertia helper按native NonDynamic编码归一，线性/角向/conditioning16组通过；去掉归一同红，完整byte恢复554回归及原startup/ragdoll11项通过 |
| R02 | `ragdoll_physics_profile::solve_profile_linear_ccd` 在相对运动读取前按本体位移退出 | 待原生确认 | 待执行 |
| R03 | 同 CCD 写回缺运动学体剩余法向位移 | 待原生确认 | 待执行 |
| R04 | 同 CCD pair filter 未消费运行期关节禁碰 | 待原生确认 | 待执行 |
| R05 | `control_rig_apply::refresh_control_rig_input` 脊柱动画时钟 | 已修复 | `e64af0759`；当前传入 `animation.delta_seconds`，不再读世界 delta |
| R06 | `als/character/movement/source_physics_timing::integrate_character_position` Air 长帧仍用末速度 | 确认并已修复 | BF135.07B：100ms六步位移`1.942775!=1.95095`红→子步速度阶段前旧速度/中点，速度补偿和clamp保持；中点、无状态terminal复算、时机三消融红，整byte恢复65相关回归；原startup/uphill/moving-base/100ms takeoff入口通过 |
| R07 | `ragdoll_link::apply_ragdoll_ground_follow` 与退出 realign 将世界位置写局部 Transform | 待原生确认 | 现 mesh affine 修复没有改角色本体的这两处写回 |
| R08 | `gen_ragdoll::target_material` 与 profile 未保留零有效休眠阈值 | 待原生确认 | 待执行 |
| R09 | `apply_profile_to_spawned_ragdoll` 将两个线性 drive target 再乘几何 scale | 待原生确认 | 待执行 |
| R10 | `locomotion_state_sync_system` 进入 Air 缺源 Rolling 中断仲裁 | 待原生确认 | 待执行 |
| R11 | `AlsGetUpState::advance` 自然零混出在窗口前置权重零 | 待原生确认 | 现 explicit-stop 剩余预算修复未涉及该分支 |
| R12 | `dispatch_graph_footsteps` 的 montage sample 缺中断状态 | 待原生确认 | 现普通 playback 中断过滤未覆盖最终图 sample |
| R13 | `bridge_roll_slot_samples` 仍写 `clock.weight=1.0` | 待原生确认 | 现 queued notify 权重修复未覆盖该 bridge |
| R14 | `source_sequence_player::next_position` 无条件取模丢闭区间末点 | 确认并已修复 | BF135.01：原生 `0!=1.5` 失败→17/17 通过→移除规则再失败→完整字节恢复同 target 17/17；原 startup/state_machine 通过 |
| R15 | `source_physics_joint::apply_linear_motor` 阻尼未反馈累计 DP/DQ | 确认并已修复 | BF135.06：8迭代`.99609375!=.5`及已有DQ力臂方向反例红→预解基线/DP-DQ反馈/源cached模式，192标量/27关节原生通过；反馈、基线无状态替代、DQ、cached模式独立消融红，源码整byte恢复554回归及原startup/ragdoll11项通过 |
| R16 | pose renderer PropertyAccess 仍输出空 `expected_tag` | 待原生确认 | 待执行；修复后必须完整转换 corpus/diff |
| R17 | state wrappers 与 GameplayTag consumer 仍采用原始大小写身份 | 待原生确认 | 待执行 |
| R18 | CCD 零允许深度仍只裁剪位置、不施加源冲量 | 待原生确认 | 待执行 |
| R19 | CCD 仍先快照所有命中再排序、未重扫已变轨迹 | 待原生确认 | 待执行 |
| R20 | dynamics Air 覆盖 Tnua 起跳帧基座 boost | 确认并已修复 | BF135.07B：实际basis+2→−2输出target+2红；首全motor方案暴露slipping误接，144.959s独立真实steep producer红→既有base上下文读取纯支持差后清理。384实际管线/65回归、pure/slip/提前清三消融与整byte恢复齐全；原四公共含18跳与100ms takeoff全部通过 |
| R21 | `next_position` 在旧时间推进后才 clamp 新 clip 长度 | 确认并已修复 | BF135.01：原生 `1!=0.875` 失败→17/17 通过→移除规则再失败→完整字节恢复同 target 17/17；原 startup/state_machine 通过 |
| R22 | `als_camera::apply_camera_view` 未将世界 view 转成渲染相机 local | 待原生确认 | 待执行 |
| R23 | Grounded CMC 子步位移历史 | 已修复 | `2f4a4d3`、`60a3287` 及 forward consumer 迁移；当前消费 `grounded_movement_delta` |
| R24 | `common_marker_names` 过早排除落选 Exclusive | 确认并已修复 | BF135.05：fresh Prepare证明全部marked player先求交，实际三角色反例先红→删提前排除，源length-sync输出通过；旧oracle源迁移，移除规则再红，字节恢复551回归及原startup/locomotion18项通过 |
| R25 | `apply_als_control_scheme_config` 使用未缩放 floating 半高 | 确认并已修复 | BF135.07C：actual scale.5 float`.924!=.474`及prephysics`4.2005687!=4.2`红→当前world纵向scale/固定clearance/源相对JumpZ，.5/1/2 geometry、16回归/384管线通过；四消融各红/整byte恢复同target、原startup/uphill18窗口通过，未改Tnua或容差 ；BF135.07D：真实base暴露mid-run scale2快速蹲起2/100，fresh CMC保持底部→现事务按当前Stance在Logic前喂Crouch，三scale100/16 jump含384管线/128动作回归绿，旧/晚阶段两消融红→整byte恢复；原startup/get_up/base/notify全通过|
| R26 | pending 后脚锁 validity | 已修复 | `79fe359d3`；当前 pending 分支同时清除 valid/became_valid/Rig validity |
| R27 | `marker_phase_sample_time_continuous` 有效缓存未对齐 leader start pair | 确认并已修复 | BF135.05：实际`.34375!=.84375`半周期反例→按方向对齐start pair，正反48步/previous time与生命周期26回归通过；移除规则同红，字节恢复551回归及原startup/locomotion18项通过 |
| R28 | `begin_source_inertialization` 对后续空 Profile 请求重复回退默认 Profile | 确认并已修复 | BF135.02：实际骨骼时长 `.25!=.1875`→修复→移除规则同红→完整字节恢复，相关101项及原 startup/state_machine 通过 |
| R29 | 缓存预热的 Additive 相关性 | 已修复 | `144b188c`、`0325fa2c`；当前 local/mesh 分支都读取有效 Alpha |
| R30 | `use_cached_context_state` 仍按首次 Use 读取认领请求 | 确认并已修复 | BF135.04：实际请求 A=.2/B=0 反例→最大权重调用上下文/普通及scoped 8组实际Pose通过；跨状态同Use ID和独立owner通过，选择/forward/重选三消融均红，完整字节恢复549回归；真实入口暴露的旋转fixture双朝向另有实际driver红绿/消融/恢复101回归，原startup/state_machine全部10项通过 |
| R31 | `SourceAnimationStateGetterNode::target_state` 按静态图定义计候选 | 确认并已修复 | BF135.03：实际Getter ambiguous→运行context绑定，保留旧scope/重入/独立linked owner均通过；作用域消融100秒旧elapsed误读再红；548回归与原startup/state_machine通过。当前比较器会回退默认值，实际症状也可为转换停住，不能照抄全部路径图失败 |
| R32 | `SourceApplyAdditiveNode::update` 未建立子 FSM Alpha 权重作用域 | 确认并已修复 | BF135.02：实际子 FSM `.4!=.1`→12组合通过；fresh source 证明 mesh 同规则，local/mesh 独立消融均红，完整字节恢复，相关101项及原 startup/state_machine 通过 |
| R33 | MachineWeight Getter 直读持久状态、缺上一完成更新发布 | 确认并已修复 | BF135.03：真实M/N第二失活更新仍Wait→发布完成权重后切到源Pose，两种执行顺序/122次失活/保留动画进度通过；current读和Temporary发布消融均失败，548回归与原startup/state_machine通过 |

验收只覆盖本次 33 项结论；未定级候选与浏览器未审范围不计入。沿用 Capture0/FunctionalOnly 的既有约定，保留性能延期，不放宽其他数值、姿态、图或 watchdog 条件。
