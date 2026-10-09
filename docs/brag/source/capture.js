// Usage:
//   node capture.js stills 1.5 4.2 ...   -> work/stills/t-<t>.png
//   node capture.js video                -> pipes every frame into ffmpeg, writes work/video-noaudio.mp4
// Playwright from a local or global install (npm i -g playwright).
const { chromium } = (() => {
  try { return require("playwright"); } catch { return require(require("child_process").execSync("npm root -g").toString().trim() + "/playwright"); }
})();
const { spawn } = require("child_process");
const path = require("path");
const fs = require("fs");

const FPS = 30;
const WORK = __dirname;

(async () => {
  const [mode, ...rest] = process.argv.slice(2);
  const browser = await chromium.launch();
  const page = await browser.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });
  await page.goto("file://" + path.join(WORK, "brag.html") + (process.env.QUERY || ""));
  await page.evaluate(async () => {
    await document.fonts.ready;
    await Promise.all([...document.images].map(i => i.complete ? 0 : new Promise(r => { i.onload = i.onerror = r; })));
  });

  if (mode === "stills") {
    fs.mkdirSync(path.join(WORK, "stills"), { recursive: true });
    for (const t of rest.map(Number)) {
      await page.evaluate(t => window.render(t), t);
      await page.screenshot({ path: path.join(WORK, "stills", `t-${t.toFixed(2)}.png`) });
    }
  } else {
    const duration = await page.evaluate(() => window.DURATION);
    const n = Math.round(duration * FPS);
    const ff = spawn("ffmpeg", ["-y", "-loglevel", "error", "-f", "image2pipe", "-framerate", String(FPS), "-i", "-",
      "-c:v", "libx264", "-preset", "slow", "-crf", "18", "-pix_fmt", "yuv420p", path.join(WORK, process.env.OUT || "video-noaudio.mp4")],
      { stdio: ["pipe", "inherit", "inherit"] });
    for (let i = 0; i < n; i++) {
      await page.evaluate(t => window.render(t), i / FPS);
      const buf = await page.screenshot({ type: "png" });
      if (!ff.stdin.write(buf)) await new Promise(r => ff.stdin.once("drain", r));
      if (i % 60 === 0) process.stderr.write(`frame ${i}/${n}\n`);
    }
    ff.stdin.end();
    await new Promise(r => ff.on("close", r));
  }
  await browser.close();
})();
