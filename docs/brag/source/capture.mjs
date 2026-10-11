// node capture.mjs stills t1 t2 ...   -> stills/t-<t>.png
// node capture.mjs video              -> video-noaudio.mp4 (30 fps, piped to ffmpeg)
import { chromium } from 'playwright';
import { spawn } from 'node:child_process';
import { mkdirSync } from 'node:fs';
const [mode, ...ts] = process.argv.slice(2);
const b = await chromium.launch();
const p = await b.newPage({ viewport: { width: 1920, height: 1080 } });
await p.goto('file://' + process.cwd() + '/brag.html' + (process.env.QUERY || ''));
await p.evaluate(() => window.ready);
const shot = () => p.locator('#c').screenshot({ type: 'png' });
if (mode === 'stills') {
  mkdirSync('stills', { recursive: true });
  for (const t of ts) { await p.evaluate(t => window.render(t), +t); await p.locator('#c').screenshot({ path: `stills/t-${(+t).toFixed(2)}.png` }); }
} else {
  const dur = await p.evaluate(() => window.DUR), n = Math.round(dur * 30);
  const ff = spawn('ffmpeg', ['-y', '-loglevel', 'error', '-f', 'image2pipe', '-framerate', '30', '-i', '-',
    '-c:v', 'libx264', '-preset', 'medium', '-crf', '16', '-pix_fmt', 'yuv420p', process.env.OUT || 'video-noaudio.mp4'], { stdio: ['pipe', 'inherit', 'inherit'] });
  for (let i = 0; i < n; i++) {
    await p.evaluate(t => window.render(t), i / 30);
    const buf = await shot();
    if (!ff.stdin.write(buf)) await new Promise(r => ff.stdin.once('drain', r));
    if (i % 60 === 0) process.stderr.write(`frame ${i}/${n}\n`);
  }
  ff.stdin.end(); await new Promise(r => ff.on('close', r));
}
await b.close();
