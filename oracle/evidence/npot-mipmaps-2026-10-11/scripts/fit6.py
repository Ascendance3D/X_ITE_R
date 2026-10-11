exec(open("fit5.py").read().split("L=load")[0])
for p in ("rr","dj"):
    L=load(p)
    for i in range(1,len(L)):
        H,W,_=L[i-1].shape
        if (H%2)!=(W%2) and min(H,W)>1:
            if W%2==0: print(p,i,f"{W}x{H} even width"); axisfit(L[i-1],L[i],"poly",1)
            else: print(p,i,f"{W}x{H} even height"); axisfit(L[i-1],L[i],"poly",0)
