import gzip
import hashlib
import json
import sys
from pathlib import Path

r=Path(__file__).resolve().parent
p=Path('tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp')
if sys.argv[1]=='restore':
    raw=gzip.decompress((r/'reader-final.cpp.gz').read_bytes())
    assert hashlib.sha256(raw).hexdigest()==json.loads((r/'reader-final-sha256.json').read_text())['sha256']
    p.write_bytes(raw)
else:
    raw=p.read_bytes()
    (r/'reader-final.cpp.gz').write_bytes(gzip.compress(raw,mtime=0))
    assert gzip.decompress((r/'reader-final.cpp.gz').read_bytes())==raw
    (r/'reader-final-sha256.json').write_text(json.dumps(dict(sha256=hashlib.sha256(raw).hexdigest(),round_trip_byte_exact=True),indent=2)+'\n')
    text=raw.decode()
    edits=[('Curves->SetNumberField(Curve.Key.ToString(), Curve.Value);','if (Curve.Key.IsNone()) { Curves->SetNumberField(Curve.Key.ToString(), Curve.Value); }'),('FProxyAccess::NativeUpdateFrame(ChildProxy));','FProxyAccess::NativeUpdateFrame(ChildProxy) * 0);'),('Request->TryGetField(TEXT("capture_proxy_curves"))->Type != EJson::Boolean || ','')]
    for old,new in edits:
        assert text.count(old)==1
        text=text.replace(old,new)
    assert text.count('if (bCaptureProxyCurves)')==1
    text=text.replace('if (bCaptureProxyCurves)','if (true)')
    p.write_text(text)
print('Proxy curve and native-frame observation: '+sys.argv[1],flush=True)
