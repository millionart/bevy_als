import sys
from pathlib import Path
r=Path(__file__).resolve().parent
mode=sys.argv[1];s=(r/'matrix.final.rs').read_bytes()
if mode=='minimax':
 old=b'let pair=source_sincos(r);';new=b'let pair=r.sin_cos();'
elif mode=='normalization':
 old=b'let same_raw_normalized=native_raw.normalize_or_zero()*Vec2::new(cx,cy).length().min(1.);';new=b'let same_raw_normalized=native_raw;'
elif mode=='angle':
 old=b'"source_angular_input":run(native_cmc).to_array()';new=b'"source_angular_input":run(semantic_current).to_array()'
else:
 assert mode=='restore';old=None
if old is not None:assert s.count(old)==1;s=s.replace(old,new)
(r/'matrix.rs').write_bytes(s);print('Prepared direction factor control '+mode,flush=True)
