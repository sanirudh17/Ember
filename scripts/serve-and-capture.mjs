import http from "node:http";
import fs from "node:fs";
import path from "node:path";
import puppeteer from "puppeteer-core";

const distDir = path.resolve("dist");
const screenshotsDir = path.resolve("screenshots");

if (!fs.existsSync(screenshotsDir)) {
  fs.mkdirSync(screenshotsDir, { recursive: true });
}

const mimeTypes = {
  ".html": "text/html",
  ".js": "text/javascript",
  ".css": "text/css",
  ".svg": "image/svg+xml",
  ".png": "image/png",
  ".json": "application/json",
};

const server = http.createServer((req, res) => {
  const url = new URL(req.url, "http://127.0.0.1:4173");
  let pathname = url.pathname;
  if (pathname === "/") pathname = "/index.html";
  const filePath = path.join(distDir, pathname);

  if (fs.existsSync(filePath) && fs.statSync(filePath).isFile()) {
    const ext = path.extname(filePath);
    res.writeHead(200, { "Content-Type": mimeTypes[ext] || "application/octet-stream" });
    fs.createReadStream(filePath).pipe(res);
  } else {
    // SPA fallback
    const fallback = path.join(distDir, "index.html");
    res.writeHead(200, { "Content-Type": "text/html" });
    fs.createReadStream(fallback).pipe(res);
  }
});

server.listen(4173, "127.0.0.1", async () => {
  console.log("Static server running on http://127.0.0.1:4173");

  const edgePath = "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe";

  const browser = await puppeteer.launch({
    executablePath: edgePath,
    headless: true,
    args: ["--no-sandbox", "--disable-gpu", "--disable-dev-shm-usage"],
  });

  const page = await browser.newPage();

  const routes = [
    { route: "overview", name: "01-overview-920x640-dark", width: 920, height: 640, theme: "dark" },
    { route: "overview", name: "02-overview-760x520-rail-dark", width: 760, height: 520, theme: "dark" },
    { route: "overview", name: "03-overview-920x640-light", width: 920, height: 640, theme: "light" },
    { route: "agents", name: "04-agents-920x640-dark", width: 920, height: 640, theme: "dark" },
    { route: "sessions", name: "05-sessions-920x640-dark", width: 920, height: 640, theme: "dark" },
    { route: "power", name: "06-power-920x640-dark", width: 920, height: 640, theme: "dark" },
    { route: "notifications", name: "07-notifications-920x640-dark", width: 920, height: 640, theme: "dark" },
    { route: "settings", name: "08-settings-920x640-dark", width: 920, height: 640, theme: "dark" },
  ];

  for (const item of routes) {
    await page.setViewport({ width: item.width, height: item.height });
    await page.emulateMediaFeatures([{ name: "prefers-color-scheme", value: item.theme }]);
    const url = `http://127.0.0.1:4173/?route=${item.route}`;
    await page.goto(url, { waitUntil: "networkidle0" });

    // Set theme explicitly on documentElement
    await page.evaluate((t) => {
      document.documentElement.dataset.theme = t;
      document.documentElement.style.colorScheme = t;
    }, item.theme);

    await new Promise((r) => setTimeout(r, 400));

    const outPath = path.join(screenshotsDir, `${item.name}.png`);
    await page.screenshot({ path: outPath });
    console.log(`Saved screenshot: ${outPath} (${fs.statSync(outPath).size} bytes)`);
  }

  await browser.close();
  server.close(() => {
    console.log("All screenshots captured and saved successfully.");
    process.exit(0);
  });
});
