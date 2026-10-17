// Order-insensitive deep compare for the Antigravity hooks.json merge.
//   node scripts/antigravity-merge-check.mjs <backup.json> <new.json>
import { readFileSync } from 'node:fs';

const [backupPath, newPath] = process.argv.slice(2);
if (!backupPath || !newPath) {
  console.error('usage: node scripts/antigravity-merge-check.mjs <backup.json> <new.json>');
  process.exit(2);
}

const before = JSON.parse(readFileSync(backupPath, 'utf8'));
const after = JSON.parse(readFileSync(newPath, 'utf8'));

if (!after.ember || typeof after.ember !== 'object') {
  console.error('FAIL: merged file has no "ember" key');
  process.exit(3);
}

const rest = { ...after };
delete rest.ember;

const canon = (v) => {
  if (Array.isArray(v)) return v.map(canon);
  if (v && typeof v === 'object') {
    return Object.fromEntries(
      Object.keys(v)
        .sort()
        .map((k) => [k, canon(v[k])]),
    );
  }
  return v;
};

const eq = JSON.stringify(canon(before)) === JSON.stringify(canon(rest));
console.log(`existing keys preserved: ${eq}`);
if (!eq) {
  console.log('--- before ---');
  console.log(JSON.stringify(canon(before), null, 2).slice(0, 2000));
  console.log('--- after (minus ember) ---');
  console.log(JSON.stringify(canon(rest), null, 2).slice(0, 2000));
  process.exit(4);
}
console.log(`ember hook keys: ${Object.keys(after.ember).join(', ')}`);
console.log('MERGE_OK');
