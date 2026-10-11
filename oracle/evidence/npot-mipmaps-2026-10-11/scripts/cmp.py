import glob, numpy as np, re, sys
for p in ("rr","dj"):
    for f in sorted(glob.glob(f"{p}.L*.rgba"), key=lambda s:int(re.search(r"L(\d+)",s)[1])):
        g = f.replace(p+".", "rs_"+p+".")
        a = np.fromfile(f,np.uint8).astype(int); b = np.fromfile(g,np.uint8).astype(int)
        d = np.abs(a-b)
        print(p, f.split(".")[1], f.split(".")[2], "max", d.max(), "mean %.3f"%d.mean(), "exact %.3f"%(d==0).mean())
