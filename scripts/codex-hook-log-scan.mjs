// Read-only scan of Codex's SQLite log for "hook" activity.
// Usage: node scripts/codex-hook-log-scan.mjs [needle] [limit]
// Default needle: "hook"; limit: 12 matches per text column.
import { DatabaseSync } from "node:sqlite";
import { homedir } from "node:os";
import { join } from "node:path";

const needle = process.argv[2] ?? "hook";
const limit = Number(process.argv[3] ?? 12);
const dbPath = join(homedir(), ".codex", "logs_2.sqlite");

const db = new DatabaseSync(dbPath, { readOnly: true });
const tables = db
  .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
  .all();

for (const { name } of tables) {
  let cols = [];
  try {
    cols = db.prepare(`PRAGMA table_info(${JSON.stringify(name)})`).all();
  } catch {
    continue;
  }
  const textCols = cols
    .filter((c) => /char|text|clob|json/i.test(String(c.type ?? "")))
    .map((c) => c.name);
  let count = 0;
  try {
    count = db.prepare(`SELECT COUNT(*) AS c FROM ${JSON.stringify(name)}`).get().c;
  } catch {}
  console.log(`\n== ${name} (${count} rows) :: ${cols.map((c) => c.name).join(", ")}`);
  for (const col of textCols) {
    try {
      const q = `SELECT ${JSON.stringify(col)} AS v, rowid AS rid FROM ${JSON.stringify(name)} WHERE ${JSON.stringify(col)} LIKE ? ORDER BY rowid DESC LIMIT ${limit}`;
      const rows = db.prepare(q).all(`%${needle}%`);
      for (const r of rows) {
        console.log(`  [${col} #${r.rid}] ${String(r.v).slice(0, 400)}`);
      }
    } catch {
      /* skip non-queryable column */
    }
  }
}
db.close();
