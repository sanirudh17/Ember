// Re-implements Codex's hook trust hash (codex-rs/config/src/fingerprint.rs +
// codex-rs/hooks/src/engine/discovery.rs) so Ember's hooks in
// ~/.codex/config.toml can be marked trusted exactly the way the TUI would
// after an interactive approval.
//
//   node scripts/codex-hook-trust.mjs          verify emulation against the
//                                              already-trusted hooks.json
//                                              entries + print Ember's hashes
//
// The stored hash is:
//   sha256:hex( canonical_json( serde_json( toml::Value( NormalizedHookIdentity ) ) ) )
// where NormalizedHookIdentity = { event_name, matcher?, hooks: [normalized] }
import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync, copyFileSync } from 'node:fs';

const CODEX_DIR = 'C:\\Users\\sanir\\.codex';
const HOOKS_JSON = `${CODEX_DIR}\\hooks.json`;
const CONFIG_TOML = `${CODEX_DIR}\\config.toml`;

const KEY_LABEL = {
  PreToolUse: 'pre_tool_use',
  PermissionRequest: 'permission_request',
  PostToolUse: 'post_tool_use',
  PreCompact: 'pre_compact',
  PostCompact: 'post_compact',
  SessionStart: 'session_start',
  SessionEnd: 'session_end',
  UserPromptSubmit: 'user_prompt_submit',
  SubagentStart: 'subagent_start',
  SubagentStop: 'subagent_stop',
  Stop: 'stop',
  Interrupt: 'interrupt',
};
const EVENTS_WITH_MATCHERS = new Set([
  'PreToolUse',
  'PermissionRequest',
  'PostToolUse',
  'PreCompact',
  'PostCompact',
  'SessionStart',
  'SessionEnd',
  'SubagentStart',
  'SubagentStop',
]);

const sha = (s) => 'sha256:' + createHash('sha256').update(s, 'utf8').digest('hex');
const sortAllObjects = (v) =>
  Array.isArray(v)
    ? v.map(sortAllObjects)
    : v && typeof v === 'object'
      ? Object.fromEntries(Object.keys(v).sort().map((k) => [k, sortAllObjects(v[k])]))
      : v;

// normalize_command_hook: SessionEnd/Interrupt default 1, clamp 1..3; others default 600, min 1.
function normTimeout(event, t) {
  if (event === 'SessionEnd' || event === 'Interrupt') {
    return Math.min(Math.max(t ?? 1, 1), 3);
  }
  return Math.max(t ?? 600, 1);
}

// hook_hash(...) with a normalized Command handler (commandWindows resolved,
// then dropped again by normalization; async always false here).
function hookHash(event, matcher, { command, timeout, statusMessage }) {
  const norm = { type: 'command', command, timeout, async: false };
  if (statusMessage != null) norm.statusMessage = statusMessage;
  const group = { hooks: [norm] };
  const m = EVENTS_WITH_MATCHERS.has(event) ? matcher : undefined;
  if (m != null) group.matcher = m;
  const identity = { event_name: KEY_LABEL[event], ...group };
  return sha(JSON.stringify(sortAllObjects(identity)));
}

// Windows resolution: command_windows wins.
const resolveCommand = (h) => (h.commandWindows !== undefined ? h.commandWindows : h.command);

// ---- load ground truth -----------------------------------------------------
const hooksFile = JSON.parse(readFileSync(HOOKS_JSON, 'utf8'));
const configText = readFileSync(CONFIG_TOML, 'utf8');

const stateRe = /\[hooks\.state\.(?:'([^']*)'|"([^"]*)")\]\s*\n\s*trusted_hash\s*=\s*"([^"]+)"/g;
const stored = new Map();
for (const m of configText.matchAll(stateRe)) stored.set(m[1] ?? m[2], m[3]);

// ---- verify against every hooks.json entry ---------------------------------
let fail = 0;
let pass = 0;
for (const [event, groups] of Object.entries(hooksFile.hooks ?? {})) {
  groups.forEach((group, gi) => {
    (group.hooks ?? []).forEach((h, hi) => {
      const key = `${HOOKS_JSON}:${KEY_LABEL[event]}:${gi}:${hi}`;
      const want = stored.get(key);
      if (!want) {
        console.log(`skip (no stored hash): ${key}`);
        return;
      }
      const got = hookHash(event, group.matcher, {
        command: resolveCommand(h),
        timeout: normTimeout(event, h.timeout),
        statusMessage: h.statusMessage,
      });
      const ok = got === want;
      if (ok) pass++;
      else fail++;
      console.log(`${ok ? 'ok  ' : 'FAIL'} ${key}`);
      if (!ok) {
        console.log(`     want ${want}`);
        console.log(`     got  ${got}`);
      }
    });
  });
}

console.log(`\nverification: ${pass} matched, ${fail} mismatched`);
if (fail > 0) {
  console.log('emulation does not match ground truth; aborting.');
  process.exit(1);
}

// ---- Ember's hooks in config.toml ------------------------------------------
// Read our commands straight from the config so hashes stay correct across
// command-shape changes (classic quoted exe vs the space-free launcher).
const ourArg = (cmd) =>
  cmd.match(/ember-hook\.cmd\s+codex\s+(\w+)/i)?.[1] ??
  cmd.match(/ember\.exe"?\s+hook\s+codex\s+(\w+)/i)?.[1] ??
  null;

const emberHooks = [];
{
  let event = null;
  for (const line of configText.split(/\r?\n/)) {
    const ev = line.match(/^\[\[hooks\.(\w+)\]\]/);
    if (ev) {
      event = ev[1];
      continue;
    }
    const cmd = line.match(/^command\s*=\s*(['"])(.*)\1\s*$/);
    if (event && cmd && ourArg(cmd[2]) && !emberHooks.some((h) => h.event === event)) {
      emberHooks.push({ event, label: KEY_LABEL[event], command: cmd[2] });
    }
  }
}
if (emberHooks.length === 0) {
  console.log('\nno Ember hooks found in config.toml; install them first.');
  process.exit(1);
}

console.log('\n== hashes for Ember config.toml hooks ==');
const stateKey = (label) => `${CONFIG_TOML}:${label}:0:0`;
const computed = [];
for (const { event, label, command } of emberHooks) {
  const hash = hookHash(event, undefined, {
    command,
    timeout: normTimeout(event, undefined),
  });
  console.log(`${label.padEnd(18)} ${hash}`);
  console.log(`  ${command}`);
  computed.push({ key: stateKey(label), hash });
}

// ---- apply ------------------------------------------------------------------
if (process.argv.includes('--apply')) {
  let next = configText;
  let changes = 0;
  for (const { key, hash } of computed) {
    const esc = key.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    const re = new RegExp(
      `(\\[hooks\\.state\\.'${esc}'\\][\\s\\S]*?trusted_hash\\s*=\\s*")[^"]+(")`,
    );
    if (re.test(next)) {
      const updated = next.replace(re, `$1${hash}$2`);
      if (updated !== next) changes++;
      next = updated;
    } else {
      next = `${next.trimEnd()}\n\n[hooks.state.'${key}']\ntrusted_hash = "${hash}"\n`;
      changes++;
    }
  }
  if (changes === 0) {
    console.log('\nall trust entries already up to date.');
    process.exit(0);
  }
  const stamp = Math.floor(Date.now() / 1000);
  const backup = `${CONFIG_TOML}.ember-backup-${stamp}`;
  copyFileSync(CONFIG_TOML, backup);
  writeFileSync(CONFIG_TOML, next);
  console.log(`\napplied ${changes} trust entries to ${CONFIG_TOML}; backup ${backup}`);
} else {
  console.log('\n(dry run; pass --apply to write the trust entries)');
}
