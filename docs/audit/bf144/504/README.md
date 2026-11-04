# BF144.504 D282 停步后的姿态与 Pole 历史

两组使用同一可见 Playground 和同一观测程序。仅 widened 组在 query 101 的两个活动方向输入使用 .503 的右向权重反事实。query 95–110 与 .503 的同组观测重合。query 139 释放输入，146 起速度为零，149 起最终 Root 姿态相同，150 起两条 Pole 阻尼状态相同，观察到 198。两组 Rig 构造计数始终为 1，两条阻尼初始化标志始终为 true。

`prepare.py` 从原始参数和 .503 的 Git 证据重新提取比较及阻尼输入。原始 JSON 可以直接读取，也可读取封存的 gzip。用 `run_replay.ps1 -Label baseline -Build` 在与观测入口相同的库上执行连续重放。只从实际 query 102 初始状态开始推进，两组共有 192 对调用，前态与结果均精确复现。在 query 146 单独将初始化标志置 false，两组直接返回同一目标。消融将 `RETAIN_WIDENED_SEED` 设为 false，重编后出现 94 处输出和 96 处前态失配。恢复原始源码字节并以 restored 标签重编，报告与基线逐字节相同。

`summary.json` 保留 Gate 结果及解压哈希，`evidence-manifest.json` 保留实际 Git 封存文件身份。
