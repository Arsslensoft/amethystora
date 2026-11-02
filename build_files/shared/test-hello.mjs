// The first login's greeting (Amethystora UI's hello module) without a screen: every language Amethystora speaks has its
// picture, each picture is what the shader reads, and the module keeps what the first look relies on. Run by 20-tests.sh:
//   node test-hello.mjs EXTENSION_DIR PO_DIR

import {existsSync, readFileSync, readdirSync} from 'node:fs';
import {inflateSync} from 'node:zlib';
import {join} from 'node:path';

const [extension, po] = process.argv.slice(2);
let failed = 0;
const check = (ok, what) => {
    if (!ok) {
        console.error(`test-hello: ${what}`);
        failed++;
    }
};

const source = readFileSync(join(extension, 'modules/hello.js'), 'utf8');

// --- The languages ---------------------------------------------------------------------------------------------------
const table = source.match(/^const WORDS = (\[.*\]);$/m);
check(table, 'the module has no table of words');
const words = JSON.parse(table[1].replace(/'/g, '"'));
const languages = words.map(([language]) => language).sort();
const spoken = ['en', ...readdirSync(po).filter(name => name.endsWith('.po')).map(name => name.slice(0, -3))].sort();
check(languages.join() === spoken.join(), `the greeting says ${languages} and Amethystora speaks ${spoken}`);
for (const [language, word] of words)
    check(word.length > 0, `${language} has no word for a screen reader`);

// --- The pictures: three measures, no transparency, which would be multiplied into them --------------------------------
const [plateWidth, plateHeight] = ['PLATE_WIDTH', 'PLATE_HEIGHT'].map(name => Number(source.match(new RegExp(`^const ${name} = (\\d+);$`, 'm'))?.[1]));
for (const language of languages) {
    const path = join(extension, 'hello', `${language}.png`);
    if (!existsSync(path)) {
        check(false, `${path} is missing`);
        continue;
    }
    const png = readFileSync(path);
    check(png.readUInt32BE(16) === plateWidth && png.readUInt32BE(20) === plateHeight, `${language}: not ${plateWidth}x${plateHeight}`);
    check(png[24] === 8 && png[25] === 2, `${language}: not 8-bit RGB`);
    check(png.length < 120000, `${language}: ${png.length} bytes`);
    // The pixels: there is ink, the pen's time spreads over the word, and ink comes with its glow
    let at = 8;
    const parts = [];
    while (at < png.length) {
        const length = png.readUInt32BE(at);
        if (png.toString('latin1', at + 4, at + 8) === 'IDAT')
            parts.push(png.subarray(at + 8, at + 8 + length));
        at += length + 12;
    }
    const rows = inflateSync(Buffer.concat(parts));
    const stride = plateWidth * 3;
    const pixels = Buffer.alloc(stride * plateHeight);
    for (let y = 0; y < plateHeight; y++) {
        const filter = rows[y * (stride + 1)];
        check(filter === 2, `${language}: row ${y} is not filtered as written`);
        for (let x = 0; x < stride; x++)
            pixels[y * stride + x] = rows[y * (stride + 1) + 1 + x] + (y ? pixels[(y - 1) * stride + x] : 0);
    }
    let ink = 0;
    let earliest = 255;
    let latest = 0;
    // Where the pen starts and where it ends, on the picture: a language read from the right starts on the right
    const first = {sum: 0, n: 0};
    const last = {sum: 0, n: 0};
    for (let k = 0; k < plateWidth * plateHeight; k++) {
        if (pixels[k * 3 + 1] > 128) {
            ink++;
            const time = pixels[k * 3];
            const part = time < 64 ? first : time > 192 ? last : null;
            if (part) {
                part.sum += k % plateWidth;
                part.n++;
            }
            earliest = Math.min(earliest, pixels[k * 3]);
            latest = Math.max(latest, pixels[k * 3]);
            check(pixels[k * 3 + 2] > 128, `${language}: ink without its glow`);
        }
    }
    check(ink > 5000, `${language}: only ${ink} pixels of ink`);
    check(earliest < 5 && latest > 240, `${language}: the pen's time runs from ${earliest} to ${latest}`);
    check(first.n > 100 && last.n > 100, `${language}: the first and last quarters of the pen's time hold ${first.n} and ${last.n} pixels`);
    const rightToLeft = language === 'ar';
    check((first.sum / first.n > last.sum / last.n) === rightToLeft,
        `${language}: written from the ${first.sum / first.n > last.sum / last.n ? 'right' : 'left'}, and ${rightToLeft ? 'Arabic goes from the right' : 'this goes from the left'}`);
}

// --- What the first look relies on, and what keeps the shell up --------------------------------------------------------
for (const [what, pattern] of [
    ['a blocking read', /_sync\(|file_get_contents/],
    ['GNOME 50\'s shader effect', /Shell\.GLSLEffect/],
    ['a window of its own', /Meta\.Window|Shell\.AppSystem|spawn/],
    ['the focus moved', /activateWindow|\.activate\(/],
    ['a birthday', /birthday|anniversary/i],
])
    check(!pattern.test(source), `the module holds ${what}`);
for (const [what, text] of [
    ['the same mark Pick a look leaves, so that a home brought along is not greeted', "'first-look.json'"],
    ['its own mark', "'hello.json'"],
    ['the hook\'s rule of a home under a day old', 'const DAY = 86400;'],
    ['the keyboard held as a modal dialog holds it, which firstlook waits for', 'actionMode: Shell.ActionMode.SYSTEM_MODAL'],
    ['any key', 'Clutter.EventType.KEY_PRESS'],
    ['a click', 'Clutter.EventType.BUTTON_PRESS'],
    ['a touch', 'Clutter.EventType.TOUCH_BEGIN'],
    ['reduced motion', "get('comfort')?.reduceMotion"],
    ['the words standing still', '_stand()'],
    ['the settings disposed', 'run_dispose()'],
    ['the still words loaded small, which keeps the module under its 2 MB', 'load_file_async(file, STILL_WIDTH, STILL_HEIGHT, 1, 1)'],
    ['the picture let go with its word', 'effect.release()'],
    ['a dialog opening over it ending it', "'notify::key-focus'"],
    ['no greeting over a dialog that is up', 'Main.modalCount === 0'],
    ['the wait for the sky over the gem flight', "actor.name === REVEAL_COVER"],
    ['the still words read from the right in a language that is', 'RIGHT_TO_LEFT'],
])
    check(source.includes(text), `the module has lost ${what}`);

// The names the transitions know it by: its freeze leaves the screen alone while the layer is up, and the greeting waits for the
// gem flight's sky
const transitions = join(extension, '..', 'amethystora-transitions@arsslensoft.github.io');
if (existsSync(transitions)) {
    check(source.includes("const NAME = 'amethystoraHello';") &&
        readFileSync(join(transitions, 'extension.js'), 'utf8').includes("actor.name === 'amethystoraHello'"),
    'the transitions no longer decline to freeze the screen over the greeting');
    check(source.includes("const REVEAL_COVER = 'amethystoraRevealCover';") &&
        readFileSync(join(transitions, 'reveal.js'), 'utf8').includes("name: 'amethystoraRevealCover'"),
    'the sky of the gem flight is not the one the greeting waits for');
}

if (failed)
    process.exit(1);
console.log('test-hello: all passed');
