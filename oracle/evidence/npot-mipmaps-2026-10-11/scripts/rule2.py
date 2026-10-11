exec(open("rule.py").read().split("data=json")[0])
data=json.load(open("synth.json"))
r=rules["current"]; seen={}
for e in data:
    w,h=e["W"],e["H"]; lv=e["lv"]; W,H=w,h
    for i in range(1,len(lv)):
        a=np.array(lv[i-1],float).reshape(h,w,4); nw,nh=max(1,w//2),max(1,h//2)
        g=np.array(lv[i],float).reshape(nh,nw,4); d=np.abs(step(a,r)-g).max()
        if (w%2)!=(h%2) and w>1 and h>1:
            seen.setdefault((w,h),[]).append((W,H,i,"BAD" if d>1 else "ok"))
        w,h=nw,nh
for k in sorted(seen):
    print(k, seen[k])
