import glob, numpy as np, re
def load(p):
    out=[]
    for f in sorted(glob.glob(f"{p}.L*.rgba"), key=lambda s:int(re.search(r"L(\d+)",s)[1])):
        w,h = map(int, f.split(".")[2].split("x"))
        out.append(np.fromfile(f,np.uint8).reshape(h,w,4).astype(np.float64))
    return out
def resample_axis(a, m, axis, kind):
    n = a.shape[axis]
    a = np.moveaxis(a, axis, 0)
    res = np.zeros((m,)+a.shape[1:])
    for x in range(m):
        if kind=="box":
            res[x] = a[min(2*x,n-1)]*0.5 + a[min(2*x+1,n-1)]*0.5 if n>1 else a[0]
        elif kind=="bilin":
            u = (x+0.5)*n/m - 0.5
            i = int(np.floor(u)); f = u-i
            i0=max(0,min(n-1,i)); i1=max(0,min(n-1,i+1))
            res[x] = a[i0]*(1-f)+a[i1]*f
        elif kind=="poly":
            if n==1: res[x]=a[0]
            elif n%2==0: res[x]=(a[2*x]+a[2*x+1])/2
            else:
                k=n//2; res[x]=a[2*x]*(k-x)/n + a[2*x+1]*k/n + a[2*x+2]*(x+1)/n
    return np.moveaxis(res, 0, axis)
def down(src, w, h, kind):
    return resample_axis(resample_axis(src, h, 0, kind), w, 1, kind)
for p in ("rr","dj"):
    L = load(p)
    for i in range(1,len(L)):
        h,w,_ = L[i].shape
        row=[]
        for kind in ("box","bilin","poly"):
            for srcname, src in (("prev",L[i-1]),("base",L[0])):
                r = np.round(down(src,w,h,kind)) if srcname=="prev" else None
                if srcname=="base": r=np.round(down(src,w,h,"bilin"))
                d=np.abs(r-L[i]); row.append(f"{kind[:4]}/{srcname}:{d.max():.0f}/{(d==0).mean():.2f}")
        print(p,i,f"{w}x{h}"," ".join(row))
