import numpy as np
f=np.float32
S1={51,59,63}
print("o  1/o*o   err(1/o) sign  k/o ...")
for o in range(3,66,2):
    inv=f(1)/f(o); p=inv*f(o); k=f(o//2)
    e1=float(inv)-1/o
    print(o, "S" if o in S1 else ".", repr(p), "%+.3e"%(e1*o), "%+.3e"%((float(f(0.5)/f(o))-0.5/o)*o), "%+.3e"%((float(k/f(o))-(o//2)/o)*o), "%+.3e"%((float(f(o//2+1)/f(o))-(o//2+1)/o)*o))
