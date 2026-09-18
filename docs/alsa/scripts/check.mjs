#!/usr/bin/env node
// Dependency-free checks for the checked-in static documentation.
import assert from 'node:assert/strict';
import {readFileSync, existsSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import vm from 'node:vm';
const root = fileURLToPath(new URL('../', import.meta.url));
const context = vm.createContext({window:{}});
for (const file of ['data.js','guides.js']) vm.runInContext(readFileSync(root+file,'utf8'),context);
const {ALSA_DOCS:data, ALSA_GUIDES:guides} = context.window;
const records = [...guides,...data.records];
const ids = new Set(records.map(r=>r.id));
assert.equal(ids.size,records.length,'Duplicate route IDs');
assert.equal(data.example,readFileSync(root+'examples/playback.c','utf8'),'Bundled example is stale; run importer');
assert.equal(data.records.filter(r=>r.id.startsWith('manual/')).length,16,'Missing PCM manual chapters');
assert.ok(data.records.filter(r=>r.id.startsWith('api/')).length>=325,'Missing API entries');
let links=0;
for (const r of records) {
  assert.ok(r.html.trim(),`Empty document ${r.id}`);
  assert.ok(!/<(?:script|iframe|object|embed)\b/i.test(r.html),`Unsafe markup in ${r.id}`);
  for(const [,href] of r.html.matchAll(/href="(#\/[^"\s]+)"/g)) {
    const [kind,name,...anchor] = href.slice(2).split('/');
    const id = kind==='reference' ? kind : kind+'/'+name;
    assert.ok(id==='reference'||ids.has(id),`Broken link ${href} in ${r.id}`);
    if(anchor.length) {
      const target=records.find(r=>r.id===id);
      assert.ok(target?.html.includes(`id="${anchor.join('/')}"`),`Missing anchor ${href}`);
    }
    links++;
  }
  for(const [,src] of r.html.matchAll(/src="([^"\s]+)"/g)) {
    if(!src.startsWith('http')) assert.ok(existsSync(root+src),`Missing asset ${src}`);
  }
  if(r.source) assert.ok(r.source.startsWith('https://www.alsa-project.org/alsa-doc/alsa-lib/'));
}
for(const name of ['snd_pcm_open','snd_pcm_set_params','snd_pcm_writei','snd_pcm_recover','snd_pcm_drain','snd_pcm_close']) assert.ok(ids.has('api/'+name));
console.log(`OK: ${records.length} documents, ${links} internal links, local assets, and bundled C example.`);
