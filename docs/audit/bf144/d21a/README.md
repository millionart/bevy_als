# BF144-D21a 空中初始穿透的直接恢复

fresh UE `SafeMoveUpdatedComponent` 默认不忽略初始穿透；`GetPenetrationAdjustment` 在 MTD 上增加 0.125 cm 并按角色/几何上限约束，若膨胀 0.1 cm 的形体在目标位置无重叠，则先移动到恢复位置再重试完整原位移。角色 CDO 的几何/角色上限为 500/100 cm。fresh 原生胶囊/墙几何探针在朝内和朝外两组均返回 20 cm MTD，外部对照命中距离 65 cm；源码、配置扫描、原生请求和结果均已保存。

目标 Falling 主扫掠先识别初始穿透，在恢复目标无阻挡时应用源 MTD 并重试；成功恢复后保留源的 `bJustTeleported` 速度语义。已写朝内/朝外、无穿透、恢复位置受阻以及加载角色事务对照。受阻时的完整源回退及侧移/补扫入口由 D21b 处理；测试、消融、图审计和真实 Playground Gate 待全部静态修复及 Bevy/依赖升级后执行。
