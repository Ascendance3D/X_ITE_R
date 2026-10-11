import json, numpy as np
def taps(n, x, shifted):
    if n == 1: return [(0,1.0)]
    if n % 2 == 0:
        return [(max(2*x-1,0),.5),(2*x,.5)] if shifted else [(2*x,.5),(2*x+1,.5)]
    k=n//2; return [(2*x,(k-x)/n),(2*x+1,k/n),(2*x+2,(x+1)/n)]
def step(a, rule):
    h,w,_=a.shape; nh,nw=max(1,h//2),max(1,w//2)
    mixed = (w%2)!=(h%2)
    sh = rule(w,h)
    out=np.zeros((nh,nw,4))
    for y in range(nh):
        ty=taps(h,y,sh and h%2==0)
        for x in range(nw):
            tx=taps(w,x,sh and w%2==0)
            out[y,x]=sum(wy*wx*a[sy,sx] for sy,wy in ty for sx,wx in tx)
    return np.round(out)
rules = {
 "current": lambda w,h: False,
 "mixed": lambda w,h: (w%2)!=(h%2),
 "mixed_both_gt1": lambda w,h: (w%2)!=(h%2) and w>1 and h>1,
}
data=json.load(open("synth.json"))
for name,r in rules.items():
    bad=[]; tot=0
    for e in data:
        w,h=e["W"],e["H"]; lv=e["lv"]
        for i in range(1,len(lv)):
            a=np.array(lv[i-1],float).reshape(h,w,4)
            nw,nh=max(1,w//2),max(1,h//2)
            g=np.array(lv[i],float).reshape(nh,nw,4)
            d=np.abs(step(a,r)-g).max(); tot+=1
            if d>1: bad.append(f"{w}x{h}:{d:.0f}")
            w,h=nw,nh
    print(name, f"{len(bad)}/{tot} bad", " ".join(sorted(set(bad))[:40]))
