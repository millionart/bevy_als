import math
import struct
f=lambda x:struct.unpack('f',struct.pack('f',x))[0]
def lerp(a,b,t):return f(a+f(f(b-a)*t))
def gait(speed):
    a,b,c=(375.,650.,2.)if speed>375 else(175.,375.,1.)if speed>175 else(0.,175.,0.)
    return lerp(c,c+1,max(0,min(1,f(f(speed-a)/f(b-a)))))
def rich(keys,time):
    if time<=keys[0]['time']:return keys[0]['value']
    if time>=keys[-1]['time']:return keys[-1]['value']
    a,b=next((a,b)for a,b in zip(keys,keys[1:])if a['time']<=time<b['time'])
    dt=f(b['time']-a['time']);alpha=f(f(time-a['time'])/dt);p0=a['value'];p3=b['value']
    if a['interp_mode']==0:return lerp(p0,p3,alpha)
    assert a['interp_mode']==2 and a['weight_mode']==b['weight_mode']==0
    p1=f(p0+f(f(a['leave_tangent']*dt)*f(1/3)));p2=f(p3-f(f(b['arrive_tangent']*dt)*f(1/3)))
    x,y,z=lerp(p0,p1,alpha),lerp(p1,p2,alpha),lerp(p2,p3,alpha)
    return lerp(lerp(x,y,alpha),lerp(y,z,alpha),alpha)
