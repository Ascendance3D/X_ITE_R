exec(open("fit.py").read().split("for p in")[0])
np.set_printoptions(precision=4, suppress=True, linewidth=150)
def fitrow(src, dst, y, ry, rx):  # rows 2y+ry.., cols 2x+rx..
    h,w,_=dst.shape; H,W,_=src.shape
    A=[];b=[]
    for x in range(1,w-1):
        for c in range(3):
            row=[]
            for dy in ry:
                for dx in rx:
                    row.append(src[min(H-1,max(0,2*y+dy)), min(W-1,max(0,2*x+dx)), c])
            A.append(row); b.append(dst[y,x,c])
    sol,res,_,_=np.linalg.lstsq(np.array(A),np.array(b),rcond=None)
    pred=np.array(A)@sol
    return sol.reshape(len(ry),len(rx)), np.abs(pred-np.array(b)).max()
L=load("rr"); src,dst=L[1],L[2]
for y in (0,1,2,50,110,111):
    k,e=fitrow(src,dst,y,range(-1,4),range(-1,3)); print("y",y,"maxerr %.2f"%e); print(k)
