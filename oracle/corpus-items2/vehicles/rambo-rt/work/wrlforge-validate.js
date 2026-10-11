#!/usr/bin/env node
'use strict';
// WRLForge v1.4.0 validation of the six shipping artifacts.
//   node work/wrlforge-validate.js <wrlforge-v1.4.0-checkout>  > qa/wrlforge-1.4.0.log
// Loads validator.js and src/vrml from the checkout given (never a bare
// require), refuses unless its package.json reads 1.4.0, and proves the gzip
// artifact decompresses to the .edit.wrl before handing the validator a
// measured size context.
const fs = require('fs'), path = require('path'), zlib = require('zlib'), crypto = require('crypto');
const WF = path.resolve(process.argv[2] || '');
const pkg = JSON.parse(fs.readFileSync(path.join(WF, 'package.json'), 'utf8'));
if (pkg.version !== '1.4.0') { console.error('refusing: WRLForge at', WF, 'is', pkg.version); process.exit(2); }
const { validate } = require(path.join(WF, 'validator.js'));
const { parse } = require(path.join(WF, 'src', 'vrml'));
const FAM = path.dirname(__dirname);
console.log('WRLForge', pkg.name, pkg.version, 'from', WF);
let bad = 0;
for (let n = 1; n <= 6; n++) {
  const slug = 'rambo-rt-' + n;
  const gz = fs.readFileSync(path.join(FAM, slug, slug + '.wrl'));
  const edit = fs.readFileSync(path.join(FAM, slug, slug + '.edit.wrl'));
  const isGz = gz[0] === 0x1f && gz[1] === 0x8b;
  const text = zlib.gunzipSync(gz);
  const match = Buffer.compare(text, edit) === 0;
  const v = validate(text.toString('utf8'), { artifactBytes: gz.length, artifactIsGzip: isGz, artifactMatchesText: match });
  const p = parse(text.toString('utf8'));
  const defs = (text.toString().match(/\bDEF\s+\S+/g) || []).length, uses = (text.toString().match(/\bUSE\s+\S+/g) || []).length;
  console.log('\n== %s  gzip %d B  sha256 %s  gzip==edit %s', slug, gz.length,
    crypto.createHash('sha256').update(gz).digest('hex'), match);
  console.log('ok =', v.ok, '| sizeStatus', v.sizeStatus, '| artifactBytes', v.artifactBytes, '| textBytes', v.textBytes);
  v.results.forEach(r => console.log('  ' + (r.pass ? 'PASS' : 'FAIL'), '[' + r.severity + ']', r.name, '::', r.detail));
  console.log('  parser diagnostics:', p.diagnostics.length, '| DEF', defs, 'USE', uses);
  p.diagnostics.forEach(d => console.log('   ', d.severity, d.code, d.message));
  if (!v.ok || p.diagnostics.length || !match || v.sizeStatus !== 'pass') bad++;
}
console.log('\nRESULT', bad ? bad + ' variant(s) not clean' : 'all six clean');
process.exit(bad ? 1 : 0);
