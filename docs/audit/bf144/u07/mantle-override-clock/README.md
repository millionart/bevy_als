# Mantle 根运动极小时钟复验

源 Clear 只清 bHasRootMotion 与 BlendWeight。Override 消费仍读取保存的 transform 平移，源 D61 已撤销。加载角色夹具取真实上一帧非零 velocity，核对 0/5/10 ns 保留速度且位置不变，以及大于阈值和正常时钟的速度与位置。focused 日志同轮包含两个尚在定位的相邻测试。
