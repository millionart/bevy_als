# BF144-D16 实体内部的地面射线

fresh Chaos 盒体射线原生探针：内部起点命中距离为 0，正常上方起点命中距离为 100 cm，盒体外平行位置无命中。ALS `ComputeFloorDist` 仅接收正时间 line hit；源代码、三组探针请求及原始结果保存在本目录。

目标 floor line 查询改为 solid 射线，使内部起点得到零距离并被现有源条件拒绝；其余 floor 查询与穿透处理不变。已写加载角色的内部、普通地面和无地面对照，并同时记录目标 solid 与 non-solid 读数。历史目标报告 `target/bf144/05/inside_ray/` 已缺失；测试、消融、图审计和真实 Playground Gate 待全部静态修复及 Bevy/依赖升级后执行。
