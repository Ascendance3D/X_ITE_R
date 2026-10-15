import { createRequire } from "node:module";
const { chromium } = createRequire ("/home/ryan/Projects/cybertown/x_ite-tests/") ("playwright");
const probe = () => {
  const bits = x => { const b = new DataView (new ArrayBuffer (8)); b .setFloat64 (0, x); return "0x" + b .getBigUint64 (0) .toString (16) .padStart (16, "0"); };
  // X3DGenerator.js createFloatFormat / createFloatExponentialFormat / FloatFormat / DoubleFormat (pinned 34cdf9f97).
  const mk = p => [ new Intl .NumberFormat ("en", { notation: "standard", maximumSignificantDigits: p, useGrouping: false }) .format,
                    new Intl .NumberFormat ("en", { notation: "scientific", maximumSignificantDigits: p, useGrouping: false }) .format ];
  const fmt = (p, v) => { const [s, e] = mk (p); if (Number .isFinite (v)) { const x = Math .log10 (Math .abs (v)); if ((p > x && x >= -4) || v === 0) return s (v); return e (v) .toLowerCase (); } return String (v); };
  const vals = [ 1, 0.9999999999999994, 0.1 + 0.2, 1.5, 2.5, 123456.789, 1e-5, 1e21, 2.5e-7, 1/3, 12345678, -0.5, 0 ];
  const out = { math: {}, double: {}, float: {}, decode: {} };
  out .math .sin_0_5 = bits (Math .sin (0.5)); out .math .cos_0_5 = bits (Math .cos (0.5));
  out .math .tan_0_5 = bits (Math .tan (0.5)); out .math .atan2_1_1 = bits (Math .atan2 (1, 1));
  for (const v of vals) { out .double [bits (v)] = [ v, fmt (15, v) ]; out .float [bits (v)] = [ v, fmt (7, v) ]; }
  const td = a => new TextDecoder () .decode (new Uint8Array (a));
  out .decode .ff_fe_fd = [...td ([0xff, 0xfe, 0xfd])] .map (c => c .codePointAt (0) .toString (16));
  out .decode .a_e2_82_c = [...td ([0x61, 0xe2, 0x82, 0x63])] .map (c => c .codePointAt (0) .toString (16));
  out .decode .bom_hello = td ([0xef, 0xbb, 0xbf, 0x68, 0x69]);
  out .decode .bom_twice = [...td ([0xef, 0xbb, 0xbf, 0xef, 0xbb, 0xbf, 0x68])] .map (c => c .codePointAt (0) .toString (16));
  out .decode .c0_80 = [...td ([0xc0, 0x80])] .map (c => c .codePointAt (0) .toString (16));
  return out;
};
const browser = await chromium .launch ({ headless: true });
const page = await browser .newPage ();
const chrome = await page .evaluate (probe);
chrome .version = browser .version ();
await browser .close ();
const node = probe (); node .version = process .version;
console .log (JSON .stringify ({ chrome, node }, null, 1));
