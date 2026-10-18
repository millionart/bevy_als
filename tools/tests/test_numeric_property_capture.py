import json
import math
import struct
import sys
from pathlib import Path

data=json.loads(Path(sys.argv[1]).read_text(encoding='utf-8-sig'))
count=0
def inspect(value):
    global count
    if isinstance(value,dict):
        if value.get('property_class')=='FloatProperty':
            text=float(value['exported_text'])
            if math.isfinite(text):
                number=value['numeric_value']
                assert math.isfinite(number)
                assert struct.unpack('f',struct.pack('f',number))[0]==number
                assert abs(number-text)<=max(5.1e-7,abs(number)*1e-6)
                count+=1
        for child in value.values():
            inspect(child)
    elif isinstance(value,list):
        for child in value:
            inspect(child)
inspect(data['blueprint_cdo_properties'])
assert count>0
print(f'Finite property values preserve native float numbers: {count} fields')
