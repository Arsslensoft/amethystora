// The Manual's moving cards: short looping clips of how a gesture or a shortcut works, drawn by us as plain shapes and
// CSS keyframes (branding/cards/<id>.html, over card.css) and turned into WebM. The only words in them are key caps
// with the keys' own names, so the clips serve every language; each has a PNG poster, its most telling frame, which
// the Manual shows with reduced motion and when the clip cannot be played.
//
// Each page is loaded in a headless Chromium-family browser through its DevTools protocol at 150 x 112 CSS pixels and
// twice the density (300 x 224), and stepped frame by frame, 30 a second for one loop, by setting every animation's
// own clock (paused), so that every frame is exactly where the loop puts it whatever the machine's speed. The frames
// are encoded with ffmpeg as VP9 (two passes, constant quality, no sound) and the script fails when a clip is over
// 300 KB or a poster over 40 KB. The outputs are committed, like the icon packs.
//
//   node branding/cards.mjs [id...]   writes system_files/shared/usr/share/amethystora/manual/media/<id>.webm and .png
//
// CARDS_BROWSER names the browser (Chromium's headless_shell, chromium, google-chrome or Edge are looked for), and
// FFMPEG the ffmpeg to use (an ffmpeg with libvpx-vp9: Fedora's ffmpeg-free has it).
import {spawn, spawnSync} from 'node:child_process';
import {copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {dirname, join} from 'node:path';
import {fileURLToPath, pathToFileURL} from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');
const PAGES = join(ROOT, 'branding/cards');
const OUT = join(ROOT, 'system_files/shared/usr/share/amethystora/manual/media');
const FPS = 30;
const SIZE = {width: 150, height: 112, scale: 2};
const CLIP_MOST = 300 * 1024;
const POSTER_MOST = 40 * 1024;

// Each card is branding/cards/<id>.html: its .art sets --loop, the seconds every animation of the page runs once in,
// and --poster, where in the loop (0 to 1) its poster is
const CARDS = readdirSync(PAGES).filter(name => name.endsWith('.html')).sort().map(name => {
    const text = readFileSync(join(PAGES, name), 'utf8');
    const loop = /--loop:\s*([\d.]+)s/.exec(text);
    const poster = /--poster:\s*([\d.]+)/.exec(text);
    if (!loop || !poster)
        fail(`${name}: .art sets no --loop or no --poster`);
    return {id: name.slice(0, -'.html'.length), loop: parseFloat(loop[1]), poster: parseFloat(poster[1])};
});

const BROWSERS = [
    process.env.CARDS_BROWSER,
    '/usr/lib64/chromium-browser/headless_shell',
    '/usr/bin/chromium-browser',
    '/usr/bin/chromium',
    '/usr/bin/google-chrome',
    'C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe',
    'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe',
].filter(Boolean);

function fail(message) {
    console.error(`cards: ${message}`);
    process.exit(1);
}

// The browser, started with its DevTools protocol on a port of its own choosing, and the address it says
async function startBrowser(profile) {
    const program = BROWSERS.find(path => existsSync(path));
    if (!program)
        fail(`no Chromium-family browser found; set CARDS_BROWSER (looked for ${BROWSERS.join(', ')})`);
    const args = ['--headless', '--remote-debugging-port=0', `--user-data-dir=${profile}`, '--no-first-run',
        '--disable-gpu', '--hide-scrollbars', '--mute-audio', '--force-color-profile=srgb', '--font-render-hinting=none'];
    if (process.getuid?.() === 0)
        args.push('--no-sandbox');
    const browser = spawn(program, args, {stdio: ['ignore', 'ignore', 'pipe']});
    const address = await new Promise((resolve, reject) => {
        let said = '';
        browser.stderr.on('data', chunk => {
            said += chunk;
            const found = /DevTools listening on (ws:\/\/\S+)/.exec(said);
            if (found)
                resolve(found[1]);
        });
        browser.on('exit', code => reject(new Error(`the browser stopped (${code}): ${said.slice(-400)}`)));
    });
    return {browser, address};
}

// One connection to the browser; each page is a session of it
function connect(address) {
    const socket = new WebSocket(address);
    let next = 0;
    const waiting = new Map();
    const listeners = [];
    socket.addEventListener('message', ({data}) => {
        const message = JSON.parse(data);
        if (message.id && waiting.has(message.id)) {
            const {resolve, reject} = waiting.get(message.id);
            waiting.delete(message.id);
            if (message.error)
                reject(new Error(`${message.error.message} (${message.error.code})`));
            else
                resolve(message.result);
        } else if (message.method) {
            for (const listener of listeners)
                listener(message);
        }
    });
    const send = (method, params = {}, sessionId) => new Promise((resolve, reject) => {
        const id = ++next;
        waiting.set(id, {resolve, reject});
        socket.send(JSON.stringify({id, method, params, ...sessionId ? {sessionId} : {}}));
    });
    const once = (method, sessionId) => new Promise(resolve => {
        const listener = message => {
            if (message.method === method && message.sessionId === sessionId) {
                listeners.splice(listeners.indexOf(listener), 1);
                resolve(message.params);
            }
        };
        listeners.push(listener);
    });
    const opened = new Promise((resolve, reject) => {
        socket.addEventListener('open', resolve);
        socket.addEventListener('error', reject);
    });
    return {opened, send, once, close: () => socket.close()};
}

async function frames(cdp, card, folder) {
    const {targetId} = await cdp.send('Target.createTarget', {url: 'about:blank'});
    const {sessionId} = await cdp.send('Target.attachToTarget', {targetId, flatten: true});
    const send = (method, params) => cdp.send(method, params, sessionId);
    await send('Page.enable');
    await send('Emulation.setDeviceMetricsOverride',
        {width: SIZE.width, height: SIZE.height, deviceScaleFactor: SIZE.scale, mobile: false});
    const loaded = cdp.once('Page.loadEventFired', sessionId);
    await send('Page.navigate', {url: pathToFileURL(join(PAGES, `${card.id}.html`)).href});
    await loaded;
    const evaluate = async expression => {
        const {result, exceptionDetails} = await send('Runtime.evaluate', {expression, awaitPromise: true, returnByValue: true});
        if (exceptionDetails)
            fail(`${card.id}: ${exceptionDetails.exception?.description ?? exceptionDetails.text}`);
        return result.value;
    };
    await evaluate('document.fonts.ready.then(() => true)');
    const loop = await evaluate('getComputedStyle(document.querySelector(".art")).getPropertyValue("--loop").trim()');
    if (parseFloat(loop) !== card.loop)
        fail(`${card.id}: the page loops in ${loop}, the table says ${card.loop}s`);
    const count = Math.round(card.loop * FPS);
    for (let i = 0; i < count; i++) {
        const at = (i * 1000) / FPS;
        // Every animation paused at this frame's time, and two frames drawn so that the picture has caught up
        await evaluate(`(() => { for (const a of document.getAnimations()) { a.pause(); a.currentTime = ${at}; }
            return new Promise(r => requestAnimationFrame(() => requestAnimationFrame(() => r(true)))); })()`);
        const {data} = await send('Page.captureScreenshot', {format: 'png'});
        writeFileSync(join(folder, `${String(i).padStart(4, '0')}.png`), Buffer.from(data, 'base64'));
    }
    await cdp.send('Target.closeTarget', {targetId});
    return count;
}

function ffmpeg(args) {
    const program = process.env.FFMPEG || 'ffmpeg';
    const result = spawnSync(program, ['-hide_banner', '-loglevel', 'error', '-y', ...args], {stdio: 'inherit'});
    if (result.status !== 0)
        fail(`${program} ${args.join(' ')} failed (${result.error?.message ?? result.status})`);
}

function encode(card, folder, count) {
    const input = ['-framerate', String(FPS), '-i', join(folder, '%04d.png')];
    const codec = ['-c:v', 'libvpx-vp9', '-b:v', '0', '-crf', '40', '-pix_fmt', 'yuv420p', '-an'];
    const log = join(folder, 'pass');
    const clip = join(OUT, `${card.id}.webm`);
    ffmpeg([...input, ...codec, '-pass', '1', '-passlogfile', log, '-f', 'webm', process.platform === 'win32' ? 'NUL' : '/dev/null']);
    ffmpeg([...input, ...codec, '-pass', '2', '-passlogfile', log, clip]);
    const poster = join(OUT, `${card.id}.png`);
    copyFileSync(join(folder, `${String(Math.min(count - 1, Math.round(card.poster * count))).padStart(4, '0')}.png`), poster);
    return {clip: statSync(clip).size, poster: statSync(poster).size};
}

const wanted = process.argv.slice(2);
const cards = wanted.length ? CARDS.filter(card => wanted.includes(card.id)) : CARDS;
if (wanted.length && cards.length !== wanted.length)
    fail(`no such card among ${CARDS.map(card => card.id).join(', ')}`);
mkdirSync(OUT, {recursive: true});
const work = mkdtempSync(join(tmpdir(), 'amethystora-cards-'));
const {browser, address} = await startBrowser(join(work, 'profile'));
const problems = [];
try {
    const cdp = connect(address);
    await cdp.opened;
    for (const card of cards) {
        const folder = join(work, card.id);
        mkdirSync(folder);
        const count = await frames(cdp, card, folder);
        const sizes = encode(card, folder, count);
        console.log(`${card.id}: ${count} frames, ${(sizes.clip / 1024).toFixed(1)} KB clip, ${(sizes.poster / 1024).toFixed(1)} KB poster`);
        if (sizes.clip > CLIP_MOST)
            problems.push(`${card.id}.webm is ${sizes.clip} bytes, over ${CLIP_MOST}`);
        if (sizes.poster > POSTER_MOST)
            problems.push(`${card.id}.png is ${sizes.poster} bytes, over ${POSTER_MOST}`);
    }
    cdp.close();
} finally {
    // The profile is written to until the browser has gone, and for a moment after by its helpers
    const gone = browser.exitCode === null ? new Promise(resolve => browser.once('exit', resolve)) : null;
    browser.kill();
    await gone;
    rmSync(work, {recursive: true, force: true, maxRetries: 10, retryDelay: 200});
}
if (problems.length)
    fail(problems.join('; '));
