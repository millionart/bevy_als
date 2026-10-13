# bevy_als — 执行清单

> 本文件是唯一权威执行清单；执行纪律见 `agents.md`（仅规则），启动方式见 `goal.md`。
> 完成一项即勾选并一次提交：实现 → 自测 → 勾选 `[x]` → `git commit -m "<前缀><编号>: <条目摘要>"`（勾选与实现同一提交）。
> 基座模块提交前缀沿用 `M<编号>`；缺陷修复清单用 `BF<编号>`。
> 已归档清单：[archivedchecklist.md](docs/archive/archivedchecklist.md)

## 背景与基础事实（已核实，持续有效）

- 本机 bevy 源码：`0.19.1`（release-0.19.1）。`bevy_animation_graph` 0.11.0 与 `bevy-tnua` 均基于 Bevy 0.19，兼容。
- bevy_animation_graph 已有节点：ClipNode、ChainNode、BlendNode（含骨骼 mask/Difference/Additive）、BlendSpace1D/2D、布尔运算节点等；图资产 RON 序列化、支持编辑器热重载；Pose 以 `BoneId` 路径表达；RootMotionMode 提取已可用（M8.8）。曲线/ramp/距离匹配等由 als-ext 经公开扩展点补齐。
- bevy-tnua 已有 basis：Walk（coyote time）、Jump（高度/悬挂控制）、Crouch、Dash、Climb、Knockback、WallSlide；物理后端 avian3d 0.12.1 / rapier3d 0.17.0。
- ALS-Refactored 关键结构：4 模块（ALS / ALSCamera / ALSExtras / ALSEditor）；状态体系用 GameplayTags（非枚举）；自定义 AnimNode_CurvesBlend；ControlRig 输入结构 `AlsControlRigInput`；IK 相关骨骼 `foot_l/foot_r`、`ik_foot_l/ik_foot_r`、`VB foot_l/VB foot_r`；关键曲线名 `LayerHandLeft/Right`、`HandLeftIk/HandRightIk`、`FootLeftIk/FootLeftLock/FootRightIk/...`；动画资产分组：Grounded / Air / Actions / Transitions / TurnInPlace / RotateInPlace / Overlays / View / Base。
- 本地环境路径、引擎版本等隐私信息见 `docs/env.md`（已 gitignore）。

## 附录 A：与 ALS 的刻意差异（非缺陷）

| ALS | bevy_als | 理由 |
|---|---|---|
| AnimBP 状态机 | 源合同生成 RON/FSM，由公开节点与 ECS 输入接线执行 | 保留源状态、转换及求值生命周期 |
| GameplayTags | `bevy_gameplay_tag` 的真实层级 Tag | 保留 UE Tag identity 与 exact/hierarchical query 语义 |
| ControlRig | 离线编译源 RigVM 程序，在源 ControlRig 节点中逐实例执行 | 保留 Construction/Forward Solve、当前 Pose/Curve 及图内求值位置 |
| Root Motion | 图内 `RootMotionMode` 提取 + `RootMotionOutput` 驱动位移（M8.8；数据经 M8.6 回填） | fork 已支持提取，M0.5 导出缺根轨道由 M8.6 离线回填，不再用曲线等价物 |
| Curves（UBT 资产） | als-ext CurveAsset + 采样节点 | 见 M2 |
| w_als_hud Overlay 菜单世界锚定头顶骨骼 | 菜单停靠 HUD 面板左侧（BF6b 用户决策 2026-08-25） | 解除 UI 对骨骼/相机投影的耦合 |
| W_Als_Hud 使用 Roboto 字体 | Bevy 默认字体（BF6b 用户决策 2026-08-25） | 字体非功能项，不移植 |
| L_Als_Playground TextRender 使用 RobotoDistanceField 与描边材质 | Bevy 默认字体布局/atlas 生成的 `Mesh3d` 世界字形四边形（BF-Scene 用户决策 2026-08-26） | 不带入字体资产；保留文字内容、颜色、世界锚点与固定朝向，不做相机 billboard |
| S_Als_UI_Click 菜单点击音 | 不复刻（BF6b 用户决策 2026-08-25） | WAV 解码需 bevy `wav` feature（hound 依赖），用户否决新增依赖 |
| UE Shift+数字仍触发玩法Action | Shift+数字用于Debug时仅抑制对应数字binding的玩法动作 | BF9用户决策（2026-08-25；4018824），手柄和非数字binding保持源行为 |

## 附录 B：风险与后置项

- **资产格式决策（2026-08-21）**：Bevy 官方推荐 glTF；M0.5/M0.6 管线定为 UE → glTF（Interchange 导出）→ bevy_animation_graph 自带的 glTF 导入器（GraphClip/Skeleton）。**不引入 bevy_ufbx**：其 v0.19.0（bevy 0.19 兼容）虽支持蒙皮数据，但动画曲线未接入 Bevy 动画系统、无社区验证，且 FBX 中转对 ALS 曲线/虚骨/单位换算风险更大；仅当未来需要直接加载 FBX 时再评估。

- bevy_animation_graph additive/mask 语义若与 ALS layering 不完全一致，在 M6 前验证并决定扩展方式（优先 als-ext 扩展点）。（已于 M6.1 验证通过，记录备查）
- Distance matching / motion matching 类高级特性仅实现 Stop 所需子集（M8.3）。
- ALSExtras（AI 控制器示例）不在范围内。
- 音频（Metasound）、Niagara 特效仅保留挂钩点。ALSCamera 完整移植已纳入 BF116.01。

## BF142 天空接入后的场景亮度与曝光

用户于2026-10-13明确要求处理天空接入后的阴沉观感。范围为原Playground天空材质输出、SkyLight环境补光、曝光与太阳Bloom显示链；截图0、性能验收暂停。先按当次本地源事实定位差异，再使用现有引擎功能，不凭观感提高太阳或添加补偿。

- [x] BF142.01 源照明/曝光与实际渲染入口静态闭环。开始：2026-10-13 08:49 +08:00；预估结束：2026-10-13 09:14 +08:00；实际结束：2026-10-13 09:13:06 +08:00；偏差−54秒，source/static Gate PASS，无需重估。依据本轮原生World读取80.603秒、原天空材质约75节点已有审计器、当前Bevy原生AutoExposure/EnvironmentMap与BF139-P2-18同类数据闭包记录，先以25分钟完成源参数/时钟/单位、HDR/材质/相机消费者、最小失败及后续可独立验收拆分。Gate：fresh World/Camera与UE实际默认、locked Bevy执行语义、必要的读工具补齐、自测及明确剩余边界；本叶不将静态通过称视觉修复。
- [ ] BF142.02 曝光/显示与环境补光修复。依赖.01 Gate，启动时按证实闭包拆为各自不超过60分钟的独立子项并登记时点。Gate：源驱动独立target数据、最小失败→修复、消融/字节恢复、同target_game正式构建、原Playground数值渲染与公共功能Gate；旧固定显示近似仅在源/症状/消费者共同证明的边界有限复开。
  - [ ] BF142.02a HDR与源参数曝光链。开始：2026-10-13 09:16 +08:00；预估结束：2026-10-13 10:06 +08:00；实际结束：2026-10-13 10:21 +08:00；偏差+15分钟，当前叶Gate PASS，未超过20分钟容差。依据.01已证参数/单位、原生AutoExposure公开组件与既有约7分钟正式构建记录，50分钟内完成独立target数据、固定曝光/contrast旧近似迁移、天空与普通材质共享HDR、focused先红/消融/恢复与原startup功能Gate。原生Bevy与UE的Histogram统计/适应和Filmic实现差异明确保留引擎边界，不声称逐像素复刻；不加入图专用补偿。
  - [ ] BF142.02b 源Bloom与太阳辉光。依赖.02a Gate。开始：2026-10-13 10:24 +08:00；预估结束：2026-10-13 11:04 +08:00；实际结束：2026-10-13 10:46:09 +0800；偏差约−18分钟，叶Gate PASS。依据fresh默认BM_SOG/强度0.675/threshold−1、六级灰色tint与已验证HDR链，使用原生Additive Bloom，离线映射源积分能量，保留两引擎blur kernel边界；40分钟闭合focused最小失败/消融/恢复、实际Bloom render pass与原manual功能Gate。
  - [x] BF142.02c 源天空捕获与环境照明。依赖.02a Gate（与Bloom叶独立），按源128捕获、已验证天空材质/2208顶点/3968三角与原生GeneratedEnvironmentMapLight闭包，在启动时拆为数据与运行两叶并各自登记不超过60分钟窗口。Gate：独立离线HDR捕获、下半球/方向/材质/生命周期反例和自测、原生IBL、消融与同target恢复、原Playground数值渲染/公共功能验收。保持原始纹理/导出不可变，不实现另一套Lumen。

    - [x] BF142.02c1 源天空捕获数据。开始：2026-10-13 10:36 +08:00；预估结束：2026-10-13 11:11 +08:00；实际结束：2026-10-13 10:53:21 +0800；偏差约−18分钟，数据Gate PASS。依据八个剩余源资产已有原生读取约5–8分钟、当次World/Material已读取且与旧记录字节相同、现有75节点求值器，35分钟闭合fresh递归数据、源坐标/网格重心UV与下半球、独立128 HDR cube、focused消融和可复跑check；本叶不称已完成运行接入。
    - [x] BF142.02c2 原生环境照明消费者与运行Gate。依赖.02c1 Gate。开始：2026-10-13 10:53:25 +0800；预估结束：2026-10-13 11:38:25 +0800；实际结束：2026-10-13 11:43:35 +0800；偏差约+5分钟，功能/数值GPU Gate PASS，未超过20分钟容差。依据已通过的cube数据、原生GeneratedEnvironmentMapLight入口、相机已有一次性marker及约6分钟正式构建记录，45分钟完成数据消费、常量Ambient迁移、原生GPU过滤与功能验收；源cube替换常量Ambient近似，原生IBL与实际GPU过滤/功能Gate、消融与同target恢复。

活动摘要（08:49）：工作区基线`93ac4a47`且干净。刚读取的World与原插件/安装副本字节一致：太阳3lux、SkyLight强度2/Stationary/CapturedScene/非实时/下半球黑，PostProcessVolume只覆盖MotionBlur；当前固定EV1002/contrast1.05来自旧显示近似，天光用常量Ambient2。下一步核对源相机后处理覆盖、曝光算法和天空进入HDR的真实路径；原始导出保持不可变。

source/static摘要（09:13）：fresh Camera Blueprint读取34.221秒证实PostProcessWeight=0，Sky Material读取69.379秒证实Surface/Opaque/Unlit；UE默认Histogram、10%–90%、速度3/1、range −10..20、18%中灰与bias1已闭合。四档均匀亮度反例证明固定EV2不能维持源中灰0.36；lens/native基准换算单元复算通过，原始记录/源码快照及哈希在`target/bf142/source_contract.json`。实际入口没有Hdr/AutoExposure，天空绕过普通材质曝光/非HDR tone mapping，SkyLight未接入。后续分别闭合曝光/HDR显示与一次性128天空捕获→原生环境照明；沿用源材质/网格算法，UE Lumen和两引擎tonemapper/filter实现差异保留能力边界，不把任意亮度补偿当源算法。当前仅source/static完成，尚未宣称观感修复。

.02a source/static摘要（09:36）：实际相机ECS合同先红（无HDR），独立target曝光导入与自测通过；源省略的bias curve/metering mask已据Scene.h明确nullptr默认处理，未补写原始记录。现有Resource改为消费离线range/filter/speed/bias/基准数据，原生AutoExposure要求HDR，contrast恢复1；两个focused相机/数值读回检测器自测通过。无需改动已验证天空材质算法，通过HDR后处理与unit曝光1共享显示链即可。GPU诊断只读ViewUniform标量，不读图像；初始化按locked Bevy RenderStartup生命周期，随后验证实际GPU曝光与消融/字节恢复。UE uniform luminance/统计与原生Bevy Rec709/统计、Filmic差异明确保留原生渲染边界，不把本项称完整UE渲染器移植；有限迁移BF-Scene-P6仅显示oracle。

.02a消融/恢复摘要（09:44）：移除AutoExposure、恢复固定EV2、丢弃源中灰/bias、恢复contrast1.05四项独立消融均语义FAIL；源码SHA-256与备份一致，自然mtime恢复，同target重编28项场景回归PASS，导入--check/self-test及资产边界PASS。诊断器的真实注册最小fixture还稳定复现PostUpdate退出晚于Update保存导致证据漏写；迁移到Last并保留退出帧自测后再正式构建。GPU资源初始化延后RenderStartup，未新增运行时曝光算法或改变原始资产/天空着色数据；下一步最终诊断自测、正常构建与公共startup数值GPU读回。

.02a静态闭合（09:47）：相机HDR/源参数/实际补偿asset合同、四个有效消融与精确恢复28项场景回归、诊断器缺失/固定/非有限值拒绝与真实注册退出保存两项自测、源数据导入self-test/check及运行时资产边界全部PASS。生产相机只复用原生AutoExposure，不增加HDR冗余组件、曝光时间层或自制算法；已验证天空shader/纹理/网格字节未改。开始同target_game/play优化构建manual_playground与更新显示oracle的scene_gate；剩余边界仅真实GPU曝光数值/公共功能Gate，天光消费者留待.02b。

.02a读回边界补闭（09:59）：首轮正式构建413.895秒PASS；回读locked Bevy发现ViewUniforms还包含阴影视图，不能默认首槽即主相机。诊断改为原生Core3d tonemapping后按实际ViewUniformOffset复制该相机数据到独立读回buffer，删除整缓冲别名路径；两项诊断自测通过。编译曾被同target的manual启动短暂占锁，确认归属后等待释放，未终止其他进程。最终工具源码清理未用import后同target正式重编，再开始一次公共startup；生产曝光算法/天空数据未再修改。

.02a runtime前（10:07）：最终同target_game/play正式构建419.375秒PASS；28项场景、两项诊断自测、四个消融/字节恢复、源数据self-test/check与资产边界闭合。开始原startup数值GPU曝光读回，复制主相机偏移，保留原地形/环境/角色/相机和所有功能守卫；截图0、性能延期。预估10:06后仍在20分钟容差内，公共Gate尚未通过，不提前核销。

.02a实际入口补闭（10:10）：首次调用被公共runner的相对输出目录约束拒绝，读入口检查后修正私有参数，无游戏启动；随后startup15.558秒功能PASS但照明证据缺失，未验收。源码定位到无driver startup不安装ManualPlaygroundIntegrationPlugin，故诊断也未安装；将同一诊断移到真实manual入口的无条件插件组，内部仍仅在显式证据路径时启用，删除driver内重复注册。生产曝光数据/算法未改，按最终入口重编后再验数值；这是工具入口漏检，不把原功能PASS替代照明Gate。

.02a第二次startup前（10:15）：最终真实入口同target_game/play重编319.070秒PASS，工具只复制实际主相机偏移，诊断无条件注册/显式路径启用和Last保存已核对。开始公共startup，必须同时通过GPU曝光标量和原功能报告；此前缺失证据的功能PASS不作照明验收。相对预估仍在20分钟容差内，不改源曝光参数或太阳强度。


.02a runtime后（10:21）：真实入口startup15.328秒，原功能Gate PASS（ready、退出码、graph/fault/temporal与primary_failure全部正常）；主相机GPU有效曝光676样本、非有限0、physical=1，EV范围−1.094..0.143，数值Gate PASS，截图0/性能延期。补充scene_gate显示计数/EV/contrast/ACES合同均通过，整体FAIL来自原运动oracle仍要求129旧曲线键及旧端点（当前源产物4键、profiles1/3），未扩展到无关运动修复；不把整体FAIL写为PASS。有限显示迁移已由focused先红/四消融/恢复28测试与真实manual标量共同闭合。用户10:16反馈太阳辉光不足，fresh UE默认Bloom已启用而目标无Bloom，纳入后续独立叶；本项完成不代表整体观感已修复。

.02b source摘要（10:24）：fresh World只覆盖MotionBlur，Scene.cpp默认BM_SOG、BloomIntensity0.675/Gaussian1/threshold−1，SceneView默认Bloom启用，目标无Bloom组件；太阳缺少屏幕散射链已有source-target根因。先补此独立显示缺口，再执行源天空捕获；新顺序不展开相邻LensFlare/LocalExposure。UE六级归一化Gaussian tint能量与原生Additive mip级联分别复算后离线映射，不直接抄强度到EnergyConserving模式，不改太阳/天空材质。

.02b source/static摘要（10:32）：源Bloom总归一化散射gain=0.08944875；原生512 mip链为8级，低频boost置0并离线解加法级联强度0.0821045966，threshold禁用与源−1一致。相机合同已稳定先红（Source bloom scattering is missing）；首次完整优化测试因只有stderr进展触发229秒watchdog，读取报告后改为已有同target focused opt0和编译目录进展，59.155秒语义FAIL。未将超时当缺组件证据。共享照明Resource直接消费源嵌套exposure/bloom数据，不增加自制render pass；实际Bloom验收复用已启用的原生GPU诊断span。尝试声明AutoExposure先于Bloom受原生private system限制，删除该配置路径，不引入按私有名称查系统或fork；两引擎histogram/Bloom调度、空间blur核与tonemapper继续作为已说明的原生渲染边界。

.02b静态闭合/runtime前（10:35）：相机Bloom最小先红/修复PASS，EnergyConserving与直接照抄0.675两项独立消融均语义FAIL（后者总scatter错误增至1.9874）；源码SHA-256精确恢复，同target28项场景回归及两项诊断自测PASS。导入self-test/check一致；生产只新增原生Bloom组件和源参数数据，复用既有GPU诊断span判定实际执行，不添加采图或自制光晕shader。开始同target_game/play正式构建，再执行原startup，必须同时满足功能、GPU曝光与native Bloom span；截图0、性能仍延期。

.02c1开始（10:36）：环境捕获与Bloom不存在算法依赖，均依赖已通过的HDR/曝光叶，源数据工作在正式构建期间独立进行。fresh World与Material已与旧记录完整SHA一致；其余BP/三色曲线/两UV网格/三纹理经同一原生reader重新读取，不因旧记录同名跳过。只派生独立捕获数据，不修改正在编译的Rust或已验证天空材质/纹理。

.02b runtime前（10:44）：正式target_game/play构建376.518秒PASS，恢复后的相机合同/28场景与2诊断测试、两项有效消融及离线数据check已通过。开始原manual startup，同时验源曝光读回、原功能报告与原生GPU Bloom span，尚不提前核销辉光叶。

.02c1 source闭合（10:44）：八个新增fresh依赖全部原生读取成功（31–38秒/资产），当次Material/World记录与前轮完整字节相同；UE捕获以SkyDistanceThreshold作为near plane，PostProcessing关闭，下半球按源上轴z<0替换黑色。采用公共source/runtime XZY坐标基与真实三角形重心插值两UV，保持原天空shader不变。求值器增加batch轴后20个既有标量输出完全一致；下一步实际128×6 half-float捕获、batch反例/消融和fresh导入check。

.02b runtime后（10:46）：原manual startup15.730秒功能PASS、GPU曝光699样本/invalid0/physical1；原生render/bloom/elapsed_gpu与tonemapping/auto_exposure均记录有效执行，截图0，性能验收延期、FullPassFalse。源驱动Bloom显示叶完成；先红/两消融/源码精确恢复/28场景与2诊断/正式同target构建/实际GPU链闭合，未将native blur核称UE逐像素等价。天空环境光叶仍未完成，继续推进。

.02c1 static/消融摘要（10:51）：fresh导入check/self-test证实原2208顶点/两UV/三纹理目标数据未变；20个标量输出原样保留，新增batch与scalar逐项一致。全128捕获23.345秒完成，786432字节、HDR峰110.6875、负Y面全黑，实际可渲染场景界限61.828米小于1500米near plane；源LowerHemisphereColor黑/alpha1、LightColor白、IndirectIntensity1已闭合。移除batch支持与下半球处理均语义FAIL，源码字节恢复后23.459秒完整重算check PASS。新捕获原点基向量fixture先红发现尚未消费的零原点字段隐藏了轴映射错误，改为公共XZY/厘米→米转换并保留非零三轴fixture；实际原点0不影响已验曝光/Bloom。4×4 fixture不覆盖窄太阳，改为只验证正辐射，太阳HDR由完整128捕获实际峰值验；不放宽实际源数据Gate。补充cube角点方向与源天光颜色合同后最后check，再完成数据叶。

.02c1完成（10:53）：最终源参数self-test/check与完整cube重算23.530秒PASS，源HDR/半球/基向量/两UV/近裁剪数据闭合，两个消融源码字节恢复后已通过；20个既有标量输出与fresh原天空数据保持一致。capture_time=0明确为场景初始化时的一次性离线快照，原生运行只消费独立target cube，不读原始记录或逐帧重捕天空。数据叶完成，运行环境照明尚未接入，不把本叶当观感修复。

.02c2 source/static摘要（10:59）：原28场景fixture先红稳定指出camera无天空照明、GlobalAmbient仍2；改为原生GeneratedEnvironmentMapLight消费独立128/RGBA16Float/Cube、源强度2/世界轴identity，旧Ambient字段由离线工具删除并置全局Ambient0。28项恢复PASS，保留payload维度/黑半球/源强度与第二帧不重建Image合同；原SkyLight实时关闭映射为一次离线时间0快照。原生GPU backend会过滤同一静态cube，不执行逐帧天空capture，未加自制capture/render状态层。诊断完整验收须同时满足曝光、Bloom、原生diffuse/specular过滤span、实际camera EnvironmentMapLight匹配和Ambient0；只有曝光成功不能让总Gate通过。

.02c2静态闭合/runtime前（11:06）：恢复旧Ambient2与遗漏源天光强度（原生默认0）两项消融均语义FAIL，生产源码SHA-256精确恢复，同target28场景与2诊断测试PASS。源数据/退休字段self-test/check通过，完整cube仍为已验128 half-float目标数据；相机第二帧不重建Image。接入只用原生组件，无新实时捕获、控制状态或依赖。开始正式target_game/play manual入口构建；真实GPU照明与公共功能Gate仍未通过，不提前核销。

.02c2真实startup前（11:14）：最终同target_game/play正式入口构建391.934秒PASS；28场景/2诊断、两消融/字节恢复、源参数与资产边界已闭合。开始公共startup，必须同时通过实际camera环境map、GPU diffuse/specular/Bloom/曝光与功能报告；截图0、性能延期。原天空材质/纹理/太阳3lux保持已证source算法，Native Perspective为infinite reverse-Z，未因名义far值展开相机改动。

.02c2首次runtime/诊断边界（11:21）：startup15.202秒原功能PASS、实际camera EnvironmentMapLight就绪1/Ambient0/Bloom/曝光有效，稳态曝光由旧Ambient时−1.163升至−0.257，约+0.906EV；总体照明Gate未通过，不能核销。fresh locked源码证实Native filter在Render/RenderSystems::Render之前，diagnostic begin_frame在后续RenderGraph中清空span，因此缺过滤计时不能代表缺过滤。删除对该计时span的验收依赖，改用原生Readback对实际diffuse/specular输出各作一次数值读取；COPY_SRC必须在native生成后的同帧、GPU资产首次prepare前配置，避免替换已绑定纹理。原生Image API解码half-float，不加依赖/自制shader，不读取最终画面。新增pending/NaN拒绝与首次生成/不重复请求fixture后重验。

.02c2数值输出补闭/runtime前（11:32）：四项诊断自测PASS（曝光缺失/固定/非有限、环境pending/NaN、原生生成初帧COPY_SRC、退出帧保存）；显式after依赖消融合并为PostUpdate阶段后信号不变，删除多余系统顺序配置。移除诊断一次性marker使请求3→5，语义FAIL，字节恢复同target重编四自测PASS。原生Image.get_color_at_3d解码过滤输出，逐像素要求finite/alpha1并记录非零RGB最大值；两张实际输出各在有效读回后清理，不存画面、不加依赖/自制shader。生产照明参数/天空数据未改，开始同target_game/play最终正式构建；剩余仅完整GPU输出与原startup/locomotion功能Gate，未核销首次证据不足的运行。

.02c2最终startup前（11:40）：最终诊断同target_game/play正式构建362.774秒PASS；新增texture读回的四自测、重复请求消融/字节恢复与生产28项场景均已闭合。开始再次公共startup，环境验收改为实际过滤输出finite/alpha1/正辐射，保留camera就绪、Ambient0、Bloom与曝光/功能守卫。当前相对11:38预估仍在20分钟容差内；截图0/性能延期，不将前次功能PASS替代GPU输出Gate。

.02c2最终startup后/locomotion前（11:42）：startup15.218秒完整照明与原功能Gate PASS；实际filtered diffuse/specular读回最大值0.512207/110.6875、全部finite/alpha1且非零，camera环境就绪1/Ambient0/Bloom/自动曝光均通过。曝光680样本、invalid0/physical1、稳态EV−0.257，原fault/graph/temporal与primary_failure正常；源太阳HDR峰完整保留，截图0/性能延期。继续原locomotion验证运动/镜头下同一HDR与环境照明消费者；无任何动画图、转换器或姿态算法修改，不展开已完成运动模块。

.02c2 runtime后/整体完成（11:43）：最终原startup15.218秒与locomotion28.279秒均完整照明/功能PASS；移动场景real_entry_paths通过，GPU曝光1530样本/invalid0/physical1，diffuse/specular有效输出峰0.512207/110.6875、camera环境就绪1/Ambient0/Bloom均通过，原graph/fault/temporal和primary_failure正常。四诊断与28场景测试、有效消融/精确字节恢复、源导入/资产边界及同target_game正式入口构建闭合；最终生产/诊断源码与恢复证据SHA一致，binary/资产哈希保留于私有final_artifact_hashes。已删除固定EV2/contrast1.05/常量Ambient2近似，使用源驱动HDR/Histogram/Bloom与一次捕获cube→原生IBL，太阳3lux及既有天空材质/纹理/动画图不变。截图0、性能验收暂停，FullPass仍False；保留原生Histogram/blur/tonemapper与UE Lumen能力边界，不声称逐像素视觉等价。补充scene_gate的旧运动oracle整体FAIL继续按.02a记录隔离，本次不改运动实现或放宽断言。各叶均已按自身Gate独立通过，BF142当前请求完成。

## BF143 角色/阴影斑驳与源显示差异

用户2026-10-13补充角色纯色表面斑驳/条纹、阴影地面同类现象，天空仍暗沉、太阳光晕偏小；本轮统一排查共用照明/显示链。保留截图0、性能验收暂停，不以改角色颜色/粗糙度或人工亮度补偿掩盖问题。

- [x] BF143.01 当次source/static与最小反例。开始：2026-10-13 13:42 +08:00；预估结束：2026-10-13 14:12 +08:00；实际结束：2026-10-13 14:12:54 +0800；偏差约0分钟，source/static Gate PASS。依据上一轮8资产31–38秒原生读取、已有源材质/捕获工具、locked原生IBL/Bloom/Histogram代码，30分钟重读World与角色材质闭包，逐项核对采样/单位/空间/曝光/显示算法，保留共享照明和光晕宽度失败反例，证明根因后有限复开BF142对应叶。Gate：fresh源事实、source-target静态合同、最小反例与不超过60分钟的后续独立拆分。

活动摘要（13:44）：基线cc0a69fe且干净，用户截图已读取。当前角色StandardMaterial纯色、roughness1、无显式texture；地面阴影同类症状将根因候选收至共享照明/后处理。Native diffuse逐texel随机半球采样，现有Bloom仅匹配DC总能量、低频boost0，旧Gate只验finite/非零/alpha1与通道执行，未覆盖方向辐射连续性、光晕空间轮廓或源显示曲线。先重新提取源材质/World与实际引擎算法，再确定有限复开边界；暂不改已验网格/动画/相机。

BF143.01 source/static闭合与有限复开（14:12）：fresh MI_Als_Character49.794秒、M_Als_Main32.385秒、World40.431秒；角色父材质只有Color向量表达式，DefaultLit/Opaque、roughness1/specular0.5/默认切线法线，目标无texture合同正确。将角色现象与阴影地面共同定位到共享环境响应，不复开角色网格/动画/材质颜色。复算locked native1024半球采样精确复现旧GPU diffuse峰0.512207；三阶SH外残差RMS0.025205，而源SH表示half量化残差0.00003908，旧finite/alpha/nonzero Gate漏掉方向连续性。UE Filmic中灰0.18→0.18，Native ACES Fitted0.18→0.105591，显示曲线直接压暗；Bloom旧DC0.08944875正确但最宽级权重2.065e−9，而源Gaussian6为0.0068625，漏检空间能量。反例和source快照索引在私有appearance_counterexamples/source_contract。仅复开BF142.02a显示曲线消费者、.02b光晕空间响应、.02c2环境过滤及其父条目；已验捕获输入/曝光单位/太阳3lux等保留，不因同名资产回改。

- [x] BF143.02 源平滑天光与实际消费者。依赖.01。
  - [x] BF143.02a 独立源IBL离线数据。开始：2026-10-13 14:12:54 +0800；预估结束：2026-10-13 14:47:54 +0800；实际结束：2026-10-13 14:32:38 +0800；偏差约−16分钟，数据Gate PASS。依据源32输入/9系数SH、已有128 captured cube与同类离线工具23秒、specular8级确定性过滤，35分钟闭合源SH/归一化/坐标/half数据、静态prefilter、连续性/常量/太阳反例与消融/check；本叶不称运行已修复。
  - [x] BF143.02b 原生EnvironmentMapLight消费与角色/阴影响应Gate。依赖.02a，开始：2026-10-13 14:32:38 +0800；预估结束：2026-10-13 15:17:38 +0800；实际结束：2026-10-13 15:17:09 +0800；偏差约−1分钟，source/GPU/功能Gate PASS。依据已验payload与现有native组件入口、focused重编/消融约5分钟、正式构建6分钟，45分钟内完成消费者与连续性数值/原入口Gate；删除逐帧随机生成路径，保持源强度/静态捕获，focused先红/消融/恢复、正式同target构建与原manual角色/地面共同照明合同。
- [ ] BF143.03 源显示曲线与光晕空间响应。依赖.01，先证明native公开LUT/blur参数能覆盖源合同，不能满足才增加最小必要实现；数据与运行分叶、分别在开始时登记不超过60分钟窗口。Gate包含源中灰/天空蓝色实际响应及完整光晕径向轮廓，不能再次只用总能量/通道执行验收。

.02a source/static与消融完成（14:32）：UE32输入逐pass half SH投影、9系数/4π面积归一化、源CPP packed Lambert归一化、source XZY坐标均复算；确定性32/64 Hammersley GGX含mip重要性采样，按native线性roughness→源log mip语义离线映射。源roughness1只使用原capture8级中的前6级，native输出6级避免把未使用的粗尾层误当roughness1；原捕获blob未改。常量辐射、法线基、Hammersley与独立方向lobe自测通过；实际diffuse与源模型最大差0.0001221≤half误差0.0002441，峰0.319336，specular峰88.3125。移除SH传递归一化、roughness映射、首层half舍入三个消融均语义FAIL，字节恢复后完整check PASS。数据叶完成，尚未接到运行，不将此结果当斑驳已修复。

  - [ ] BF143.03a 源Filmic LUT与光晕空间数据。开始：2026-10-13 14:40:02 +0800；预估结束：2026-10-13 15:30:02 +0800。依赖.01，与.02b运行消费者独立；依据原生48³ LUT公开接口、已证源Filmic/blue correction/gamut扩展与六Gaussian层，50分钟完成源码驱动离线色彩响应、native blur完整空间响应拟合能力判定及反例/消融。source合同不能覆盖时先登记最小必要运行实现分叶，不以人工曝光/亮度补偿代替。

.02b静态闭合/runtime前（15:02）：实际相机缺少直接EnvironmentMapLight的最小反例已先红；消费源SH diffuse32与6级radiance、保留强度2/世界轴，删除GeneratedEnvironmentMapLight。MipMajor→LayerMajor消融语义FAIL，源码字节恢复同target28场景与4诊断测试PASS；GPU读回新增对实际上传payload逐字节核对，防止随机结果凭finite/非零再次通过。开始同target_game/play正式入口构建，尚未核销运行叶。

.02b runtime前（15:08）：正式同target_game/play构建353.435秒PASS；28场景/4诊断、MipMajor有效消融和精确恢复已闭合。开始原manual startup，必须验证直接EnvironmentMapLight、无随机生成器、实际GPU diffuse/specular与源payload逐字节一致，以及原功能守卫；截图0/性能延期。显示曲线/Bloom另叶仍在推进，不能把本叶称整体观感完成。

.02b startup后/locomotion前（15:15）：原startup15.733秒完整GPU/功能PASS，直接EnvironmentMapLight1、无随机生成器、Ambient0，实际diffuse/radiance与源payload逐字节一致，峰0.319336/88.3125；曝光612样本invalid0/physical1。sourceSH方向连续性已由数据合同覆盖，开始原locomotion验证活动角色/镜头消费者；截图0/性能延期。

.03a source能力判定（15:15）：原生48³公开LUT接口可消费源Filmic数据；中灰直接源0.180001、native LUT0.179492，天空/角色/高亮样本最大误差0.00326，黑值极限与省略的WhiteTint/FilmBlackClip按fresh Scene.cpp默认闭合。Native Bloom FIRST_DOWNSAMPLE固定Karis压缩均匀HDR输入（1→0.9293、110→16.123），源DownsampleHigh为线性四次双线性平均；任意固定native强度/低频参数不能同时满足两档输入线性合同。故放弃native参数拟合，后续最小源Gaussian扩展只负责六级线性downsample/源核/叠加，继续复用native HDR/曝光/LUT显示。

.02b runtime后/完成（15:17）：原startup15.733秒与locomotion28.963秒GPU/功能PASS，实际direct EnvironmentMapLight1、无随机生成器、GPU diffuse/radiance每字节匹配源离线数据，峰0.319336/88.3125，曝光1370样本invalid0/physical1，原graph/fault/temporal与primary_failure正常。源SH/法线方向连续性合同、颜色/粗糙度不变、28场景/4诊断、有效消融/字节恢复及同target正式构建闭合；BF142.02c/c2有限复开完成。角色和阴影共用的噪声来源已替换；天空显示曲线和光晕仍为当前.03，不提前宣称整体完成。截图0，性能验收延期。
