# BF144-D15b 双墙调整与第二次滑动扫掠

fresh UE `UMovementComponent::SlideAlongSurface` 在首次滑动命中后执行 `TwoWallAdjust`，只在方向显著且仍朝向原请求时再扫掠一次。CMC 的上坡限制允许接触法线或几何 `ImpactNormal` 满足可走条件。fresh 原生逆向坡样本在第 40 帧记录四次碰撞；高坡对照引用 D15a。两个原始请求、读数、源码片段和 reader 身份可重验。

目标将回退滑动和 StepUp 内部滑动合并为同一两次扫掠流程，第二次命中使用已有源几何面选择；首次扫掠初始穿透仍按原 StepUp 回退处理。已写几何面资格对照、第二方向被第三面墙阻挡对照，并保留原有 StepUp 双墙和 D15a 多子步测试。测试、消融、图审计和真实 Playground Gate 待全部静态修复及 Bevy/依赖升级后执行。
