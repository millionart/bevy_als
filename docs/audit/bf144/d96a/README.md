# BF144-D96a 静摩擦锚点保存与匹配合同

fresh PA确认摩擦历史恢复和simple匹配开启，exact/near距离分别为0.2/1cm。额外调用仓库原生PhysicsContactProbe，取得8组4、6、8、14点的真实保存及下一帧匹配数据；请求、reader报告、无损压缩原始结果及压缩前后字节哈希已保存。PA输出与D65a记录逐字节相同。

目标独立实现源ratio保留/插值/零ratio丢弃规则，以及按quadratic侧、循环起点和strict exact/near距离选择锚点。执行 `python3 tools/import/import_contact_history_settings.py --facts docs/audit/bf144/d96a/source-facts.json --native docs/audit/bf144/d96a/native-history-settings.json --output src/als/settings/contact_history.rs` 可重建目标距离与ratio阈值；执行 `python3 tools/import/export_contact_history_probe.py --source docs/audit/bf144/d96a/history.native.json.gz --output docs/audit/bf144/d96a/history.runtime.json` 可重建单位/坐标转换后的对照数据。

已写全部原生样本的保存/匹配与精确距离、quadratic侧、7/8点和零ratio边界测试。代码尚未接入接触生命周期；测试、消融与Playground待统一Gate。
