// Preserve the user-supplied brand artwork. Never procedurally replace it.
import { execFileSync } from "node:child_process";
import { copyFileSync } from "node:fs";
execFileSync(process.platform === "win32" ? "npx.cmd" : "npx", ["tauri", "icon", "app-icon.png", "--fit", "contain"], { stdio: "inherit", shell: process.platform === "win32" });
copyFileSync("src-tauri/icons/32x32.png", "src-tauri/icons/tray.png");
copyFileSync("app-icon.png", "src/assets/ember.png");
