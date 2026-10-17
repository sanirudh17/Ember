// Wait for the Ember dev app's CDP endpoint to accept connections.
//   node scripts/wait-cdp.mjs [timeoutSeconds]
const DEBUG = 'http://127.0.0.1:9333';
const timeout = Number(process.argv[2] ?? 90) * 1000;
const t = Date.now();
for (;;) {
  try {
    const targets = await (await fetch(`${DEBUG}/json`)).json();
    console.log(`CDP_UP targets=${targets.length}`);
    process.exit(0);
  } catch {
    if (Date.now() - t > timeout) {
      console.log('CDP_TIMEOUT');
      process.exit(1);
    }
    await new Promise((r) => setTimeout(r, 1500));
  }
}
