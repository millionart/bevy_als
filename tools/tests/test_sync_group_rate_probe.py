import json
import struct
import sys
from pathlib import Path

f32=lambda value:struct.unpack('f',struct.pack('f',value))[0]
sequence=json.loads(Path(sys.argv[1]).read_text(encoding='utf-8-sig'))['sync_group_clock_probe']
assert len(sequence['players'])==1 and sequence['players'][0]['duration']==1
frames=sequence['samples'][0]['frames']
assert len(frames)==5
time=f32(0.2)
for frame,rate in zip(frames,[1,2,0,-1,0.5]):
    previous=time
    movement=f32(f32(0.1)*rate)
    time=f32(time+movement)
    assert frame['leader']==0 and frame['order']==[0]
    player,=frame['players']
    assert player['position']==time and player['previous']==previous
    assert player['playback_delta']==movement
if len(sys.argv)>2:
    blend=json.loads(Path(sys.argv[2]).read_text(encoding='utf-8-sig'))['sync_group_clock_probe']
    assert len(blend['players'])==1 and blend['players'][0]['time_unit']=='normalized'
    frames=blend['samples'][0]['frames'];assert len(frames)==5
    for frame in frames:
        assert frame['leader']==0 and frame['order']==[0]
        assert len(frame['players'][0]['samples'])==1
    samples=[frame['players'][0]['samples'][0] for frame in frames]
    assert frames[2]['players'][0]['position']==frames[1]['players'][0]['position']
    assert samples[2]['time']==samples[1]['time']
    assert frames[3]['players'][0]['position']<frames[2]['players'][0]['position']
    assert abs((samples[1]['time']-samples[1]['previous_time'])-0.05)<1e-6
    assert abs((samples[4]['time']-samples[4]['previous_time'])-0.0125)<1e-6
print('Native per-step rates, pause, reverse and configured-rate fallback PASS',flush=True)
