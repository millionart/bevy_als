# BF144-D125a 胶囊与箱体正时间扫掠

`source-facts.json` 固定本轮源文件身份和专用扫掠、形体尺寸、单纯形及坐标转换的相关事实。`native-fixtures.json` 选取 521、577、578 已跟踪的 14 个正时间原生胶囊扫掠，包含原始输入、输出和原文件 SHA-256。对应 Rust 合同比较距离、接触点和接触法线，并保留反向扫掠控制；按本轮要求尚未运行。

在仓库根目录可用以下命令从已跟踪的原生报告重建输入：

```sh
python3 - <<'PY'
from pathlib import Path
import gzip, hashlib, json
samples = []
for number in ('521', '577', '578'):
    base = Path('docs/audit/bf144') / number
    path = base / 'geometry.native.json'
    if not path.exists():
        path = base / 'geometry.native.json.gz'
    packed = path.read_bytes()
    raw = gzip.decompress(packed) if path.suffix == '.gz' else packed
    for entry in json.loads(raw)['geometry_query_probe']['samples']:
        result = entry.get('capsule_sweep', {})
        if result.get('hit') and result.get('distance_cm', 0) > 0:
            samples.append({'source': str(path), 'source_sha256': hashlib.sha256(packed).hexdigest(),
                            'input': entry['input'], 'output': result})
Path('docs/audit/bf144/d125a/native-fixtures.json').write_text(json.dumps({'samples': samples}, indent=2) + '\n')
PY
```

零时间命中的穿透深度、全场景候选排序和真实消费者接入均属于 D125b；本子项不改变当前生产查询，也不代表 D125 已完成。
