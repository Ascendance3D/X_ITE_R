#!/usr/bin/env python3
"""REA batch 08 lane `net` oracle baselines (pinned X_ITE, headless Chrome on the GPU).

  python3 -I oracle/tools/rea_b08_net.py shots [--only SUBSTRING]
      oracle/synthetic-rea-b08-net/*.wrl -> oracle/render-rea-b08-net-baseline (x_ite-shot.mjs) and
      oracle/render-msaa4-rea-b08-net-baseline (x_ite-shot-msaa.mjs, 4 samples).
  python3 -I oracle/tools/rea_b08_net.py case NAME WORLD SCRIPT MSAA [TEXT_TOL] [--udp]
      x10_compare.py against the release build in $B08_NET_TARGET (default /tmp/b08/net/target/release), plus
      the equality of the sorted warn / error `log` of both sides (x10_compare does not look at it). On success
      the X_ITE side (js.json, js/*.png) is kept as oracle/rea-batch08-net-baseline/NAME-msaaMSAA and the case line is
      added to oracle/rea-batch08-net-baseline/cases.txt.
      --udp: a loopback sender (127.0.0.1 only) sends DIS-PDU-shaped datagrams to port 41234 (the `port` of the
      reader nodes) and 41235 (nobody) during the whole run, and a listener on 41236 (the `port` of the writer
      nodes) records anything the nodes send. Both X_ITE and Rust must leave 41234 closed (connected UDP sockets
      get ECONNREFUSED) and the listener empty; udp.json records the counts.

A baseline is written only from an X_ITE run; the Rust result is compared, never copied.
"""
import json, os, shutil, socket, struct, subprocess, sys, threading, time

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
args = sys.argv[1:]
SENDER_PORTS = (41234, 41235)
LISTEN_PORT = 41236


def pdu(kind, size):
    """DIS PDU header (IEEE 1278.1: version 6, exercise 1, type, family, timestamp, length) plus filler."""
    family = {1: 1, 25: 4, 26: 4, 27: 4}[kind]
    body = bytes(range(256)) * ((size // 256) + 1)
    return struct.pack(">BBBBIHH", 6, 1, kind, family, int(time.time()) & 0xFFFFFFFF, size, 0) + body[:size - 12]


class Udp:
    def __init__(self):
        self.stop = threading.Event()
        self.sent = {p: 0 for p in SENDER_PORTS}
        self.refused = {p: 0 for p in SENDER_PORTS}
        self.received = 0
        self.control = None
        # refusals per half second window and port: a socket that is bound (a node that listens) would show 0.
        self.windows = {p: {} for p in SENDER_PORTS}
        self.t0 = time.time()

    def run(self):
        # Positive control: a bound socket receives what the sender sends, so a silent run means something.
        s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        s.bind(("127.0.0.1", SENDER_PORTS[0]))
        s.settimeout(2)
        c = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        c.sendto(pdu(1, 144), ("127.0.0.1", SENDER_PORTS[0]))
        try:
            self.control = len(s.recvfrom(2048)[0]) == 144
        except OSError:
            self.control = False
        s.close()
        c.close()
        self.listener = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self.listener.bind(("127.0.0.1", LISTEN_PORT))
        self.listener.settimeout(0.05)
        socks = {}
        for p in SENDER_PORTS:
            k = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
            k.connect(("127.0.0.1", p))
            socks[p] = k
        kinds = (1, 25, 26, 27)
        i = 0
        while not self.stop.is_set():
            for p, k in socks.items():
                try:
                    k.send(pdu(kinds[i % 4], 144 + 8 * (i % 5)))
                    self.sent[p] += 1
                except ConnectionRefusedError:
                    self.refused[p] += 1
                    self.sent[p] += 1
                    w = int((time.time() - self.t0) * 2)
                    self.windows[p][w] = self.windows[p].get(w, 0) + 1
            i += 1
            try:
                data = self.listener.recv(2048)
                self.received += 1
            except OSError:
                pass
            time.sleep(0.01)

    def start(self):
        self.t = threading.Thread(target=self.run, daemon=True)
        self.t.start()
        time.sleep(0.3)

    def finish(self):
        self.stop.set()
        self.t.join()
        self.listener.close()
        n = int((time.time() - self.t0) * 2)
        quiet = {p: [w for w in range(1, n - 1) if not self.windows[p].get(w)] for p in SENDER_PORTS}
        return {"control": self.control, "sent": self.sent, "refused": self.refused, "windowsWithoutRefusal": quiet, "listenerReceived": self.received}

    @staticmethod
    def verdict(r):
        ok = bool(r["control"]) and all(r["sent"][p] >= 20 and r["refused"][p] >= 5 and not r["windowsWithoutRefusal"][p] for p in SENDER_PORTS) and r["listenerReceived"] == 0
        return ok


if args[0] == "shots":
    only = args[args.index("--only") + 1] if "--only" in args else None
    for tool, out in [("x_ite-shot.mjs", "render-rea-b08-net-baseline"), ("x_ite-shot-msaa.mjs", "render-msaa4-rea-b08-net-baseline")]:
        cmd = ["node", os.path.join(root, "oracle", tool), os.path.join(root, "oracle", "synthetic-rea-b08-net"), os.path.join(root, "oracle", out)]
        if only:
            cmd += ["--only", only]
        subprocess.run(cmd, check=True, cwd=root, timeout=900)
        # viewAll worlds animate over frames: a still shot (frame 0) is not a comparison, the stepped cases cover them.
        for f in os.listdir(os.path.join(root, "oracle", out)):
            if f.startswith("dis-viewall"):
                os.remove(os.path.join(root, "oracle", out, f))
elif args[0] == "case":
    udp_on = "--udp" in args
    args = [a for a in args if a != "--udp"]
    name, world, script, msaa = args[1:5]
    tol = args[5] if len(args) > 5 else None
    out = f"/tmp/rea-b08-net-case/{name}-msaa{msaa}"
    shutil.rmtree(out, ignore_errors=True)
    cmd = ["python3", os.path.join(root, "oracle", "tools", "x10_compare.py"), world, script, out, "--target", os.environ.get("B08_NET_TARGET", "/tmp/b08/net/target/release")]
    if msaa != "0":
        cmd += ["--msaa", msaa]
    if tol:
        cmd += ["--text-tol", tol]
    u = Udp() if udp_on else None
    if u:
        u.start()
    r = subprocess.run(cmd, cwd=root, capture_output=True, text=True, timeout=600)
    udp = u.finish() if u else None
    print("\n".join(l for l in r.stdout.splitlines() if l.startswith("FAIL") or "within tolerance, " in l or l.startswith("PASS both") or l.startswith("    ")))
    code = r.returncode
    try:
        js, rs = json.load(open(os.path.join(out, "js.json"))), json.load(open(os.path.join(out, "rs.json")))
        same = js.get("log") == rs.get("log")
        print(("PASS " if same else "FAIL ") + f"warn/error log equal ({len(js.get('log', []))} lines)")
        if not same:
            print("    js:", js.get("log"), "\n    rs:", rs.get("log"))
            code = code or 1
    except Exception as e:
        print("FAIL no result", e)
        code = code or 1
    if udp is not None:
        ok = Udp.verdict(udp)
        print(("PASS " if ok else "FAIL ") + f"udp {json.dumps(udp)}")
        code = code or (0 if ok else 1)
    print(f"exit {code}")
    if code == 0:
        dest = os.path.join(root, "oracle", "rea-batch08-net-baseline", f"{name}-msaa{msaa}")
        shutil.rmtree(dest, ignore_errors=True)
        os.makedirs(dest)
        shutil.copy(os.path.join(out, "js.json"), dest)
        if os.path.isdir(os.path.join(out, "js")):
            shutil.copytree(os.path.join(out, "js"), os.path.join(dest, "js"))
        if udp is not None:
            json.dump(udp, open(os.path.join(dest, "udp.json"), "w"), indent=1)
        cases = os.path.join(root, "oracle", "rea-batch08-net-baseline", "cases.txt")
        line = " ".join([name, world, script, msaa] + ([tol] if tol else []) + (["--udp"] if udp_on else []))
        lines = open(cases).read().splitlines() if os.path.exists(cases) else []
        lines = [l for l in lines if l.split()[:1] != [name] or l.split()[3:4] != [msaa]] + [line]
        open(cases, "w").write("\n".join(lines) + "\n")
    sys.exit(code)
