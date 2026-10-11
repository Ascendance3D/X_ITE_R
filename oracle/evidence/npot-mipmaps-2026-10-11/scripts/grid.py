exec(open("rule.py").read().split("data=json")[0])
data=json.load(open("synth.json")); bad={}
for e in data:
    w,h=e["W"],e["H"]; lv=e["lv"]
    a=np.array(lv[0],float).reshape(h,w,4); nw,nh=max(1,w//2),max(1,h//2)
    g=np.array(lv[1],float).reshape(nh,nw,4)
    d0=np.abs(step(a,rules["current"])-g).max(); d1=np.abs(step(a,rules["mixed"])-g).max()
    ev,od=(w,h) if w%2==0 else (h,w)
    bad[(ev,od,w%2==0)] = "." if d0<=1 else ("S" if d1<=1 else "?")
evs=sorted({k[0] for k in bad}); ods=sorted({k[1] for k in bad})
for orient in (True,False):
    print("even is width" if orient else "even is height"); print("  e\\o "+"".join(f"{o%100:>3}" for o in ods))
    for e in evs: print(f"{e:5} "+"".join(f"{bad[(e,o,orient)]:>3}" for o in ods))
