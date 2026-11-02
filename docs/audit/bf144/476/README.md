# BF144.476：实际腿IK输入/输出与公开单调用

原生数据与时点合同先由476a封存，见 FOUNDATION.md / foundation-summary.json（提交6326eb51）。本项完成原完整范围：1332源调用、29304 pin、1272 active current腿调用的完整参数及三骨T/Q/S单调用重放、必要输入反例与同target源码字节恢复。Source隐式长度只确认输入前提，source数学求解仍是下一依赖。

两条current 319完整历史增加的post-IK getter运行在hierarchy克隆上，完整旧register/pose/call/trace保持。共享函数在初始帧也调用零权重arms，部分arms有第四个PoleSpace key；共2548观察，1272是本项实际腿，不把其它调用充作active手臂算法验收。

单调用保持current原initial globals、精确三骨before/current参数，1272×22参数after及三骨global全值精确。临时强制seed+1位置再恢复这一步去掉后完整报告byte相同，已删。Wrong post-pose作为before仍重现1272输出，是满权重求解幂等无反例；因此输入时点由source function/pc/typed参数契约证明：1332 pre接受、1332 post拒绝。不可只凭最终输出相等宣称读到了正确输入。

初版pole清零先被旧参数回显拒绝，不能证明pose必要性。修正反例对修改后参数作echo检查，echo通过后1272 pose全部不同，最终合同101拒绝；首例三骨值保留parameter-negative.json。按replay.rs.final源码字节恢复、同target/六库重编后，1272完整报告与基线及seed简化结果逐字节相同。没有把source post结果注入current求解输入。

全部长期native、请求、源事实、完整输入/差表、失败恢复、阶段Gate、身份与manifest都在Git目录。构建二进制仅缓存；截图0、性能暂停。整个source solver、上游pose生产和完整Rig/Root未在本项通过。

项目根复验：run_python.ps1 -Name prepare / check_current / stage_contract；run_current_vm.ps1 -Label <new> -Build运行两条完整当前VM；run_replay.ps1 -Label <new> -Build进行完整单调用；analyze.py保全部source/current pre/post差异，不冒称solver归因。No-pole-pose-input与旧echo-only/幂等尝试分别保留为不同源码与报告；结束后恢复final字节并同target编译。facts/identity依本地配置与ALS_UE_SOURCE_ROOT，所有自动流程使用已self-test的固定wall/no-progress watchdog。
