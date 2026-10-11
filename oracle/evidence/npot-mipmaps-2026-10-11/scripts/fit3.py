exec(open("fit.py").read().split("for p in")[0])
L = load("rr")
src, dst = L[1], L[2]   # 400x225 -> 200x112
h,w,_ = dst.shape
cand = resample_axis(resample_axis(src,h,0,"poly"),w,1,"box")
# check row and column shifts and transposition-like hypotheses
for dy in range(-2,3):
  for dx in range(-2,3):
    a = dst[max(0,dy):h+min(0,dy), max(0,dx):w+min(0,dx)]
    b = cand[max(0,-dy):h+min(0,-dy), max(0,-dx):w+min(0,-dx)]
    print(dy,dx, "%.2f"%np.abs(a-b)[..., :3].mean())
print("dst row0", dst[0,:4,:3].tolist()); print("cand row0", cand[0,:4,:3].tolist())
print("dst last", dst[-1,:4,:3].tolist()); print("cand last", cand[-1,:4,:3].tolist())
# is dst a flipped image?
print("flipY err %.2f"%np.abs(dst[::-1]-cand)[..., :3].mean())
