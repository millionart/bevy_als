# BF144-D58 Mantle 初始角色完整变换

fresh ALS 在根逆变换后乘完整 ActorTransform，再转换到目标基座空间；RMS 保存位置与旋转，插值后才提取 twist。UE VQS 对正缩放先缩放根逆位移，再由角色完整旋转变换。fresh Low Right 根轨道采样、请求、reader 身份和相关源码保存在本目录。

手动和自动入口现传入完整世界 Transform；基座相对转换保留初始化所需缩放，初始根变换在首次基座刷新前准备。已写正均匀/非均匀缩放、pitch/roll、基座变化及加载角色入口对照；测试、消融和动画 Gate 待统一验证。负缩放涉及 UE 的矩阵回退，未在本项复测；旧清单所引 `target/bf144/18/initial_controls/` 已不存在，其原始报告缺失。
