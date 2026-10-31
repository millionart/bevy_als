本目录为 BF144.442b 的 Git 长期证据。方向基由现有 coordinate_basis 工具核对：source X→runtime X、Y→Z、Z→Y；方向无长度缩放。source相机前向X与runtime相机前向−Z不同，物理等价视角为 φ=−θ−π/2。共同旋转算子控制会换输入轴与符号，仅用于隔离数学，不冒充相同Forward/Right动画语义。

`native.json` 的77控制与 `actual.native.json` 的240实际相机/输入参数构成317原生数学样本。独立source producer复算double轴clamp、float degree/radian/minimax、double旋转输入及CMC clamp，各阶段逐值原生同；它执行已核源算法的Core API对照，不是原角色输入事件/相机模拟。

`actual-consumers.json` 与 `summary.json` 分离D269源minimax、D270额外世界normalize、D271角度表示阶段。原生向量小于单位长度而source不normalize时，F64额外normalize仍不符，故不是仅F32精度问题。实际240仅21/22方向ULP不等，公开核计划仅22保留差，21相消；不声称最终body反馈或完整Root恢复。另两斜向clamp余差在同wide算法下只换中间F32投影即可复现，按已知D266保留。

`ablate.py minimax|normalization|angle|restore` 生成受控源码，再以 `run_matrix.ps1 -Label <新标签> -Build` 重编后运行新标签。移除minimax应在native pair断言失败，其余两个控制应丢对应信号；源码bytes恢复并同target重编，完整317输出byte同。没有修改生产代码。首次消费者编译语法失败后误运行旧二进制的输出已单列invalid，所有消费者结论只取成功新编译后的输出。

正式输入、原生报告、源码、数值、身份及消融结果均随Git保存；编译缓存与普通进程日志可临时放tools/tmp。完整角度范围、非相机/首次fallback生命周期、实际物理反馈和连续Root仍不在本项完成声明内；父项442未完成。
