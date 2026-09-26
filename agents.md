# bevy_als — agents.md（执行规则）

用 Bevy ECS 复刻 ALS-Refactored（UE 5.8）的高级运动系统。以 `bevy_animation_graph` 与 `bevy-tnua` 为基座；不做网络同步。

**当前工作范围由 [`CHECKLIST.md`](CHECKLIST.md) 与 [`TODO.md`](TODO.md) 决定。** `CHECKLIST.md` 保留当前执行批次，`TODO.md` 保留用户明确迁入的待办；待办条目本身不授权调研或实现，直到用户将其选为当前任务。本文件规定跨任务纪律；动画图专属流程见 [`skills/animation-graph-equivalence/SKILL.md`](skills/animation-graph-equivalence/SKILL.md)。

## 规则归属

| 范围 | 归属 |
|---|---|
| 清单、提交、安全、source-first、真实入口、跨引擎语义 | 本文件 |
| UE 图取证、RON/FSM 对标、图编辑器、动画运行时 Gate | animation-graph-equivalence skill |
| 可重复的静态审计、导入、视觉报告与自测 | `tools/` 对应工具 |

## 执行纪律

1. **清单顺序与提交**：按当前任务所在的 `CHECKLIST.md` 或 `TODO.md` 的模块和依赖顺序执行；每项通过自身 Gate 后勾选并单独提交。Gate 未通过不得开始依赖项。
2. **活动条目摘要与范围**：每完成一轮连贯的 source/static 闭环，以及每次真实 runtime Gate 前后，都在当前条目所在文档下补一条简短摘要：已覆盖的 source 闭包、已通过的审计/命令、当前未通过边界与下一步。未经 source 证明且未在该摘要中说明的相邻图、consumer 或辅助链路不得展开；完成条目不得因猜测复开。
3. **待执行与已完成项隔离**：登记未来条目不授权调研或实现。已完成或已归档条目只有在症状、fresh source fact 和 source-target 链路共同证明它是根因时才能复开；先取消勾选、复盘旧 Gate 的漏检边界，再改代码。不得因相邻模块、同名资产或猜测而回头修改。
   每次修复转换器或共享转换规则后，必须对全部已生成AnimBP及其递归依赖执行全量转换，再用完整新旧产物diff确认影响范围，统一核对、有限复开和迁移；不得预先按当前图或节点类型缩小转换集合，已完成项隔离不能成为保留旧规则产物的理由。同一节点类型使用唯一转换规则，节点ID、所属图和字段取值只作为数据，不得产生图专用算法。
4. **归档**：只移动明确命名且已完成/废止的完整清单段落。归档前说明移动与保留边界，核对 diff；`docs/archive/archivedchecklist.md` 只作日期索引，归档单独提交。
5. **设计消融**：每次新增抽象、适配层、状态、工具步骤或运行时分支都要在 focused fixture、测试或受控路径中移除/合并并重跑适用 Gate；信号不变即删除，只保留有明确失败边界的最小实现。
6. **通用与项目边界**：动画图扩展放 `crates/als-ext/src/animation_graph_ext`；Tnua 控制器适配放 `crates/als-core/src/tnua_ext`，项目物理时钟接线放 `src/tnua_ext`。Control Rig、Avian、Bevy、ALS 和 Playground 分区见 `crates/als-ext/README.md`，不得继续把新模块堆回 crate 根目录。ALS 规则、资产选择和角色生命周期留在 ALS 分区或项目/als-core；文件归位不等于已解除 ALS 依赖。先用项目公开扩展点，不默认改依赖、fork 或 vendor 源码；确实无法满足时单独记录阻塞和维护代价。
7. **估时与超时复盘**：新增清单项时必须按对标 source/技术文档事实、递归闭包规模和已归档同类任务的实际记录估时，并写明开始时间与预估结束时间；预估结束时间超过开始后 60 分钟的工作先拆成可独立验收的子项。预估结束允许 20 分钟误差；超过该误差仍未通过 Gate 时，不得搁置、悄然延后或直接沿用失效估时，必须暂停实现和新的 runtime 启动，回读当前条目、source closure 与证据，摘要已耗时工作、漂移/阻塞和纠正方向，登记下一轮开始/预估结束时间后自动继续同一任务。每次重新估时均以复盘后的剩余工作为依据，按上一轮估时窗口的 1.5 倍推导新的时间点；清单只记录开始/预估结束时点，不以具体时长充当拖延空间。连续重估不得无限延长：同一项目累计超过三轮预估仍未完成时，第四轮开始前必须重新审阅整个过程、重构问题拆分和验证路径，再建立下一轮估时。该检查点不是任务终止、等待用户批准或缩小目标的理由；只有权限缺失、真实外部阻塞或用户明确停止才可不继续。每次完成、超时或拆分都在当前/归档条目记录开始时间、预估结束时间、实际结束时间、偏差、完成/失败结果与拆分结果；将这些记录按同类任务回顾，持续校正后续估时，使成功和超时都成为提高估时准确度的能力数据。

## Source、资产与移植

1. **source-first**：对标修改前必须从本地 ALS-Refactored 工程重新提取当次事实；本地路径可用时不得用网页版本替代。事实和 Gate 结论写入当前条目或提交说明，禁止凭记忆实现。
2. **只复刻已证实差异**：观察、截图和性能样本只用于定位。先逐项比较 source/target 的算法、输入、值、生命周期和坐标空间；source 未证实差异或 source 自身相同则不做优化、补偿、平滑或阈值放宽。
3. **原始导出不可变**：UE 导出的原始资产是 source record，不追加、重写或手工修补。单位、坐标基和算法标量的 target 数据必须由可复跑离线工具生成到独立 target 资产/配置；运行时只消费 target 数据。迁移已完成条目的原始资产边界必须先显式复开该条目。
   纹理按Texture.Source实际通道数选择容器（4通道TGA、3通道PNG），导出后重新解码并逐像素核对，禁止按名称、扩展名或Alpha内容推断通道数。材质移植须用`tools/import/audit_material_texture_channels.py`核对源图所需通道→实际文件通道→shader采样；源需要A而目标缺A必须失败并追查导出工具，不能补造Alpha。打包纹理直接采样，保留丢Alpha与漏shader通道的最小反例。
4. **Transform 与单位**：位置、旋转、scale、轴、手性、parent space、骨骼/IK/root motion/trace 等均先推导完整坐标基和单位语义，再转换；不能只抄 XYZ 或套通用 Rotator。转换工具使用中性 source/runtime 命名，并以基向量、旋转和字段复算验证。
5. **生命周期**：明确一次性初始化、条件重建、事件和逐帧阶段。逐帧 pose/IK 写回从当帧动画输入开始，禁止回读上一帧 post-rig Transform 形成累积；重建须有触发、清理和执行次数证据。
6. **物理时钟**：按 source 与 target 的实际执行语义映射 Delta clamp、fixed accumulator、substep 和异步路径；不得凭配置名或 `1/30` 把物理限制改成全局固定帧率。

## 验收与真实入口

1. **静态闭环优先**：source fact → source/target 静态合同 → focused test/self-test → 编译与声明式 Gate → 一次真实 Playground runtime Gate。真实运行是静态闭环后的兜底，不是逐笔探索工具。
2. **真实场景**：运行时功能必须接入 `run_manual.bat` 的可见 `manual_playground`；自动场景保持原 Playground 地形、环境、角色和相机，不能用场外、空白或隔离世界替代。
3. **动画图与角色姿态**：修改图、状态机、曲线、IK、Control Rig、root motion 或资产选择时，必须使用 animation-graph-equivalence skill 的公共视觉 Gate。图求值错误必须在首帧生成可读报告并退出：报告至少包含原始错误、pin/node、候选图路径、root-pose/A-pose 分类、场景标签和截图尝试；runner 即使进程非零退出也必须审计该报告。没有 `primary_failure` 的 graph-error 报告视为无效。
4. **经验固化**：修复后优先把漏检边界落实为 source-first 断言、真实入口场景、工具 self-test 或适用 skill；跨任务纪律写本文件，领域流程写 skill。经验是经证实的默认，不是死规则；source、工具或运行时事实变化时用新证据调整。
5. **自动化进展 watchdog**：任何自动运行的场景、测试或外部工具流程都必须定义可记录的进展信号与固定无进展上限；同一进展位置超过上限时，自动结束而非无限等待，并将最后进展、等待时长、当前条件和退出原因写入机器可读结果。watchdog 本身必须有 focused self-test。

## 工程安全

1. **隐私**：提交内容不得含本地绝对路径、账户或引擎位置；用 gitignored 配置、环境变量或参数注入。提交前检查暂存 diff 的路径泄露。
2. **代码纪律**：可执行源码、字符串、日志、断言和资产语义字段只用英文且不写注释；Markdown 可使用中文。
3. **构建**：大型 cargo 命令必须限流（本机通常 `--jobs 2`）；每个 worktree 使用自己的 target，并用 `pwsh tools/new-worktree.ps1 -Name <name>` 创建。运行/性能 Gate 的预构建必须使用启动器实际使用的同一 `CARGO_TARGET_DIR`（`run_manual.bat` 为 `target_game`）；默认 target 的构建不能充当该入口的二进制证据。排查锁前先确认进程和 target 归属，不能误杀其他 worktree 或 rust-analyzer。
4. **UE/uasset**：禁止使用 UE MCP。通过本地 UE 源码实现、维护并使用仓库的 uasset 读取工具，直接提取资产、图、属性与递归依赖；优先复用已有原生读取能力，缺失字段先对照 UE 源码补齐工具，再重新读取，不能用猜测或手工补写代替源事实。
5. **工具调用**：UE 源资产统一经仓库 uasset 读取工具调用。复杂长命令改为短命令加脚本；超时或错误时先读取报告、改变诊断路径，不盲目重复同一调用。

## 文档与历史

`CHECKLIST.md` 保留当前执行目标、依赖和最小 Gate 摘要；`TODO.md` 保留已迁入的待办；归档保留完成或终止证据；skill 保留可复用方法；工具与 self-test 保留可执行细节。不要把整段历史、失败截图路径或一次性数字重复复制到所有位置。
