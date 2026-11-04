# BF144.547 正接触法线的面资格

已 Walking 角色在 q40 以同一 50 ms 输入撞低凸台，顶板限制跨步抬升，随后胶囊撞 43 cm 竖直墙的上沿。两案只改顶板下表面高度 3 cm。高顶板案的接触法线竖直分量约 0.774–0.777，高于可走阈值。低顶板案约 0.674–0.675，低于阈值。

UE 前向命中按 ImpactNormal 选竖直侧面，两案均压平滑动，随后从前向停止点实际下扫。同一上沿在向下扫掠时按顶面判可走。两案均提交跨步并保持 Walking。

Bevy 高顶板案按胶囊接触法线保留向上滑动，导致下扫起点相对 UE 多前进约 74 mm、偏高约 60 mm。只读重放仅替换此前向命中的面资格，起点余差降至 0.02 mm 内。低顶板案的两条重放逐值相同。两案功能 Gate 通过，真实物理解算运行生产路径。证据归入 BF144-D10 的跨步滑动消费者。截图为 0，性能验收暂停。

封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/547/run_python.ps1 -Name verify`。重采使用空输出目录，原生依次运行 `prepare_request`、`prepare_reader`、`build.ps1 -Label edge`、`native.ps1 -Label edge -Request edge`。目标依次运行 `observer`、`compile.ps1 -Label edge`、`playground.ps1` 的 `upper` 与 `lower` 及各自的 `functional.ps1`。通用 profile、Rig、骨架和自测复用 522 的相同文件。
