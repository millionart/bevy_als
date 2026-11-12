# D149 出生后的首个 Jump 复验

旧目标真实角色用例在 Tnua Logic 前已收到跳键边沿。角色为 Grounded、位置高度 0.9214999 m、出生标志已释放，动作仍未开始。锁定的 Tnua 0.33.2 只有启用或 SenseOnly 才进入 `apply_controller_system`。首次缺少地面传感器时，`get_or_create_sensors` 创建实体并把动作处理推迟一帧。

角色在出生等待阶段使用 Tnua 的 SenseOnly 与原有刚体停用。地面传感器创建后，下一普通帧可以完成源输入、传感器、动作及马达顺序。世界步长为零时，动画可先完成一次注册姿态，保持原 pending，等可创建传感器后再启用物理。

fresh Locomotion 资产复核原 T19 与 T20。首普通帧已实际起跳，注册 pending 让该帧图选 Fall。下一普通帧才发起跳时选 Jump。核验同时保留零步长等待中的一次注册求值。

原始出生失败输出、fresh 原生资产和重放与本目录同存。原生资产采用无损 gzip，解压后 SHA-256 见 `source-identity.json`。
