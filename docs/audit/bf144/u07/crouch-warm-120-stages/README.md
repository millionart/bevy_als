# 蹲行首差的物理及曲线阶段

复用上一轮 0.8 幅值、1.2 m/s 蹲行和完整 89 行。原启动地面拟合值约为 0.001 m，出生点下方实际碰撞射线命中 0 m。第一次普通步由 Air 事务正确移动约 5.45 mm，Avian 求解阶段另外下移约 0.182 mm，第二步没有这段附加移动。各阶段位置、速度、重力和马达输出在 `stages.json` 与完整记录中。

源 Root 首次出现 PoseGait 时，目标 Standing 的 Forward BlendSpace 输出为 1，后继方向 Blend Multi 为 0.9999998807907104。相同目标值随后也出现在 Crouching 的方向 Blend Multi。完整观察包含活动图节点 GUID，可与已安装图和同输入原生读数配对。

复跑沿 `../crouch-warm-120-latest/README.md` 的临时钩子及本目录观察器。钩子已从正式入口按字节移除。
