exec(open("fit.py").read().split("for p in")[0])
import itertools
for p in ("rr","dj"):
    L = load(p)
    for i in range(1,len(L)):
        h,w,_ = L[i].shape
        best=[]
        for ky,kx in itertools.product(("box","bilin","poly"),repeat=2):
            r=np.round(resample_axis(resample_axis(L[i-1],h,0,ky),w,1,kx))
            d=np.abs(r-L[i]); best.append((d.max(),ky,kx,(d==0).mean()))
        best.sort(); print(p,i,f"{L[i-1].shape[1]}x{L[i-1].shape[0]}->{w}x{h}",best[:2])
