# BF144.546 压平滑动前的旧地面法线代换

已 Walking 角色在 q40 以同一 50 ms 输入撞低凸台，抬升后撞同一俯倾墙。两案使用 ±4° 的旧地面，方位角、角色起点和 1.8 cm 起始间隙相同。UE 两案的旧地面都有阻挡，墙接触法线的竖直分量约 −0.5。

上坡时，前向请求与旧地面接触法线点积为负。UE 先用旧地面法线替代墙面法线，再压平并滑动。下坡时点积为正，UE 压平墙面法线。两案由实际墙面命中、旧地面和剩余时间复算，得到的位移均等于墙面停止点到真实 StepDown 起点的位移。UE 均实际下扫命中凸台并保持 Walking。

Bevy 实际在两组保留朝下滑动。上坡组仅压平墙面的只读重放仍留下 74.91 mm 的水平下扫起点偏差，按源条件代换地面后降到 2.58 mm。固定同一目标剩余请求时，实测重放与原生投影公式相差小于 0.03 mm。下坡组两条重放逐值相同。原生首次命中前已沿旧坡移动，目标仍水平，记录了这段 D13 差异与后续余差。功能 Gate 两案通过，物理解算运行生产路径。证据登记为 BF144-D302。

封存复验运行 `pwsh -NoProfile -File docs/audit/bf144/546/run_python.ps1 -Name verify`。重采使用空输出目录，原生依次运行 `prepare_request`、`prepare_reader`、`build.ps1 -Label floor`、`native.ps1 -Label floor -Request floor`。目标依次运行 `observer`、`compile.ps1 -Label floor`、`playground.ps1` 的 `opposed` 与 `following` 及对应的 `functional.ps1`。通用 profile、Rig、骨架和自测沿用 522 的相同文件。截图为 0，性能验收暂停。
