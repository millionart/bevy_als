# BF144.565 起跳同帧的基座速度所有者

真实角色先站稳旧平台。q49 跳键时，旧平台和新平台分别提供 +1 m/s 与 −1 m/s 的水平速度。两案只改新平台是否在同帧抬高到更近的传感器位置。起跳前角色的自身速度为零。

原生两案在跳键前都绑定旧平台，实际进入 Falling 时均继承 +1 m/s。原生竖直起跳速度为 4.2 m/s，帧末水平向正方向移动约 5 cm。

Bevy 两案的上一帧基座也都是旧平台。未抬高案的当前传感器仍选旧平台，实际马达与原生同为 +1 m/s。抬高案的传感器及 Tnua 基座记忆改选新平台，实际马达继承 −1 m/s。竖直结果相同，水平终点相对原生落后约 10 cm。证据登记为 BF144-D312，针对跳键帧的速度实体选择。两组功能 Gate 通过，截图为 0，性能验收暂停。

封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/565/run_python.ps1 -Name verify`。重采使用空输出目录，原生依次运行 `prepare_request`、`prepare_reader`、`build.ps1 -Label jump`、`native.ps1 -Label jump -Request jump`。目标依次运行 `observer`、`compile.ps1 -Label jump`、`playground.ps1` 的 `switch` 与 `same` 及各自的 `functional.ps1`。
