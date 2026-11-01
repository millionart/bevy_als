import sys
from pathlib import Path

r=Path(__file__).resolve().parent;p=r/'inventory.py';saved=(r/'inventory.py.final').read_bytes()
if sys.argv[1]=='restore':
    p.write_bytes(saved);assert p.read_bytes()==saved
else:
    assert sys.argv[1]=='remove'and p.read_bytes()==saved
    text=saved.decode();old="f32=lambda x:struct.unpack('f',struct.pack('f',x))[0]";assert text.count(old)==1
    p.write_text(text.replace(old,'f32=lambda x:x'))
print('Applied standalone inventory projection control '+sys.argv[1],flush=True)
