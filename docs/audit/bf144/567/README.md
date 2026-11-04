# BF144.567 Walking 换台后旧台晚到位移

同一 100 ms 帧从旧台跨至加宽的新台，两个子步后仍 Walking。两组只切换旧台是否在运动事务后平移 20 cm。UE 在运动更新回调中已绑定新台，平移旧台不会携带角色。目标事务末端 floor 已是新台，物理后仍用旧台携带，再发布新台绑定。旧台静止时两端横向位置一致。

归并 BF144-D313 的基座提交时机。566 核对离地速度，567 核对仍 Walking 时的物理后位置跟随。D36 的跳键顺序另有记录。首轮 PostUpdate 帧索引偏一，作废读数保留在 `first-run-frame-alignment.json`。纠正后分别在物理后和物理前注入，最终以物理前结果与 UE 运动回调比对。

复验运行 `pwsh -NoProfile -File docs/audit/bf144/567/run_python.ps1 -Name verify`。重采使用空输出目录，原生依次运行 `prepare_request`、`prepare_reader`、`build.ps1 -Label late`、`native.ps1 -Label late -Request late`。目标依次运行 `observer`、`compile.ps1 -Label late`，再运行 `playground.ps1` 的 `late` 与 `still` 和对应的 `functional.ps1`。截图 0，性能验收暂停。
