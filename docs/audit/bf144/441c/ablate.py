import sys
from pathlib import Path
r=Path(__file__).resolve().parent
mode=sys.argv[1];s=(r/'kernel.final.rs').read_bytes()
if mode=='unit':
    a=s.index(b'let native=GroundedDynamics{');b=s.index(b';let data=',a);s=s[:a]+b'let native=si'+s[b:]
elif mode=='key':
    old=b'corrected.acceleration.grounded.normal.acceleration.keys[2].leave_tangent=-3642.3720703125f32/100.;';assert s.count(old)==1;s=s.replace(old,b'corrected.acceleration.grounded.normal.acceleration.keys[2].leave_tangent=params.acceleration.grounded.normal.acceleration.keys[2].leave_tangent;')
elif mode=='order':
    old=b'let si=dyns(&row["source_order_si"]);';assert s.count(old)==1;s=s.replace(old,b'let si=current;')
else:assert mode=='restore'
(r/'kernel.rs').write_bytes(s);print('Prepared controlled kernel variant '+mode,flush=True)
