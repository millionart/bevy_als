本目录为 BF144.441c 的长期 Git 证据。`summary.json` 限定通过范围，`evidence-manifest.json` 绑定文件字节。source240 使用已提交的 441b 新原生事实；target240 来自原 manual Playground 的真实角色。它们各自产生输入，世界原点和轨迹不同，不能把两者逐帧数值差全算作同输入缺陷。

`build.ps1` 在项目根目录按原启动器 play/target_game/jobs2 构建。`prepare_observer.py` 在不改生产源码的前提下，复制原入口并加入明确的控制输入、时钟和被动观察；`compile_observer.ps1` 使用 `library-identities.json` 绑定的六份实际库。完整场景、角色和相机保留；摘录不是未经改动公共入口验收的替代。

`startup.ps1` 对应独立公共 startup 功能 Gate；截图为0，性能失败保留。`runtime-assets-root/parameters.json` 为完整240帧消费输入；首轮 `runtime-baseline` 因直接启动的资产根错误在0帧被90秒无进展watchdog终止，修正仅为标准 BEVY_ASSET_ROOT 环境。

`prepare_cases`、`kernel.rs`、`analyze_kernel` 绑定实际计划与公开核，并分离当前算序、source算序下的米制/厘米求值和原键控制；`mapping.native.json` 保留720原生映射与240同向量长度。`ablate.py unit|key|order|restore` 生成控制源码，每次用 `run_kernel.ps1 -Label <新标签> -Build` 重编，再执行对应新标签；`check_controls` 检查本轮完整恢复结果。

所有长期输入、源码、数值、机器报告与身份均在本目录。可执行文件和原始编译/进程日志临时位于 tools/tmp；重新运行使用新证据目录，保留本次原始报告。当前窗口没有证明完整物理反馈、连续Root或全部动画输入等价；旧12018身份链仍缺失，不能宣称重验了全部旧产物。
