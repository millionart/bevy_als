# D124 Raw 虚拟骨骼

两向 GetUp 和 SprintAcceleration 使用相同的 SKM_Als 原生采样容器。绑定顺序来自原生骨架。已生成的虚拟骨骼参与后续绑定的组件空间计算。

```sh
python tools/import/gen_als_virtual_bones.py --mesh docs/audit/bf144/d124/mesh.native.json --sequence docs/audit/bf144/d124/front.native.json --sequence docs/audit/bf144/d124/back.native.json --sequence docs/audit/bf144/d124/sprint.native.json
python docs/audit/bf144/d124/replay_virtual_bones.py
```

比较报告含原生 Raw 重放、冻结依赖消融、原目标 glTF 采样和重算，以及原生 Compressed 数据的对照。
