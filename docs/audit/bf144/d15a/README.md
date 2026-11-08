# BF144-D15a 跨步回退后的首次滑动扫掠

fresh UE `SlideAlongSurface` 在跨步被拒或失败后先按接触法线、几何面资格和当前 floor 限制滑动方向，再对滑动请求执行 `SafeMoveUpdatedComponent`。fresh 原生高坡场景先命中侧凸台，接着实际扫向斜坡，并在后续命中后保持 Walking；原始帧、请求、源片段和诊断 reader 身份保存在本目录。

目标回退结果使用首次滑动扫掠的实际位移，单步 CMC 直接消费已扫掠结果，避免交给物理阶段重放；多子步也从实际停点继续。已写双墙、上半球拒绝、无第二墙以及加载角色多子步边界对照。首次滑动再命中后的 TwoWallAdjust 与第二次扫掠留给 D15b；测试、消融、图审计和真实 Playground Gate 均待全部静态修复及 Bevy/依赖升级后执行。
