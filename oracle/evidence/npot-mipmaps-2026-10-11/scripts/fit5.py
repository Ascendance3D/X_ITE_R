exec(open("fit.py").read().split("for p in")[0])
np.set_printoptions(precision=3, suppress=True, linewidth=150)
def axisfit(src,dst,known_axis_kind,fit_axis):
    # filter the other axis with known kind, then fit taps on fit_axis per output index
    h,w,_=dst.shape
    if fit_axis==1: V=resample_axis(src,h,0,known_axis_kind); V=np.moveaxis(V,1,0); D=np.moveaxis(dst,1,0)
    else: V=resample_axis(src,w,1,known_axis_kind); D=dst
    n=V.shape[0]; m=D.shape[0]
    for x in list(range(0,3))+[m//2]+list(range(m-3,m)):
        cols=[c for c in range(2*x-2,2*x+4) if 0<=c<n]
        A=np.stack([V[c][...,:3].ravel() for c in cols],1); b=D[x][...,:3].ravel()
        sol=np.linalg.lstsq(A,b,rcond=None)[0]; e=np.abs(A@sol-b).max()
        print(f"  out {x}: src {cols[0]}..{cols[-1]} w={sol} err {e:.2f}")
L=load("rr")
print("rr L1->L2 width even 400->200 (height poly)"); axisfit(L[1],L[2],"poly",1)
print("rr L1->L2 height odd 225->112 (width box)"); axisfit(L[1],L[2],"box",0)
D=load("dj")
print("dj L2->L3 height even 84->42 (width poly)"); axisfit(D[2],D[3],"poly",0)
