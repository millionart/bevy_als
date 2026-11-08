# BF144-D14 角色胶囊的禁止跨步默认值

fresh 角色 CDO 的 `CollisionCylinder.CanCharacterStepUpOn=ECB_No`，Playground 七个 AI 角色实例的同一属性也全部为 `ECB_No`。UE `ACharacter` 设置该默认值，CMC 通过命中组件的 `CanCharacterStepUp` 消费；fresh ALS Mantle 前向检测也检查同一权限。本目录保存两个原生资产读取、七个实例身份、相关源代码和 reader 身份。

目标 `AlsCharacter` 默认附带现有 `AlsMantleTargetPermission { allows_step_up: false }`，显式组件仍可覆盖，普通物体仍使用原有允许默认值。Mantle 权限查询现在包含角色，以便同一源属性能被该消费者读到；子碰撞体仍先查自身，再回退到所属刚体。已写默认与覆盖、加载角色运动事务、普通物体、子碰撞体及 Mantle 检测对照。历史目标报告 `target/bf144/05/permission/` 已缺失；测试、消融、图审计和真实 Playground Gate 待全部静态修复及 Bevy/依赖升级后执行。
