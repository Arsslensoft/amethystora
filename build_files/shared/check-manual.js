'use strict';

// Checks the manual's pages for 20-tests.sh: every page in pages.json exists and has a title, and
// every link between pages lands on a page that is listed and on a heading that page has. The same for
// each translation, manual/<language>/<page>.md: it translates a listed page, and its links land on
// the headings of the English pages, whose ids a translated page keeps (apps/manual). One whose headings
// no longer match the English page's is only reported, since a link into it opens the English page at that
// section (apps/manual), and Weblate brings it back in line at its next sync.
// Run on the Node that 20-tests.sh installs for it and check-translations.js, and takes out again:
//
//   node check-manual.js [manual dir] [--links page#heading...]
//
// Links from outside the manual, such as Control's cards (amethystora-control-list --json), are given
// after --links in the form `amethystora-manual` takes, and each has to land the same way.

const fs = require('node:fs');
const path = require('node:path');

const linksAt = process.argv.indexOf('--links');
const outside = linksAt >= 0 ? process.argv.slice(linksAt + 1) : [];
const manual = (linksAt === 2 ? null : process.argv[2]) || '/usr/share/amethystora/manual';
const toc = JSON.parse(fs.readFileSync(path.join(manual, 'pages.json'), 'utf8'));
const pages = new Map();
const problems = [];

// The same ids the Manual gives headings (markdown::slug in apps/common), from their text with the Markdown
// taken out: what HTML escapes left out, and every run of what is neither a letter (Unicode's Alphabetic,
// which takes in Arabic's vowel marks) nor a number one dash
function slug(heading) {
    return heading.replace(/`/g, '').replace(/\*\*?/g, '').replace(/\[([^\]]*)\]\([^)]*\)/g, '$1')
        .replace(/&[#\w]+;/g, '').replace(/[&<>"']/g, '').toLowerCase().replace(/[^\p{Alphabetic}\p{N}]+/gu, '-')
        .replace(/^-|-$/g, '');
}

for (const group of toc) {
    for (const file of group.pages) {
        const id = path.basename(file, '.md');
        const text = fs.readFileSync(path.join(manual, file), 'utf8').replace(/^```[\s\S]*?^```/gm, '');
        if (!/^# .+/m.test(text)) {
            problems.push(`${file}: no "# " title`);
        }
        if (pages.has(id)) {
            problems.push(`${file}: a second page called ${id}`);
        }
        const anchors = new Set([...text.matchAll(/^#{2,6} (.+)$/gm)].map((m) => slug(m[1])));
        pages.set(id, { file, text, anchors });
    }
}

function checkLinks(file, text) {
    for (const [, href] of text.matchAll(/\]\(([^)\s]+)\)/g)) {
        if (/^(https?|mailto):/.test(href)) {
            continue;
        }
        // Show me (R7-61): check-guides.mjs holds each to a guide that exists
        if (href.startsWith('guide:')) {
            continue;
        }
        // A moving card (apps/manual/src/cards.rs): one of the manual's own clips, with its poster beside it
        if (href.startsWith('media/')) {
            const card = /^media\/([a-z0-9-]+)\.webm$/.exec(href);
            if (!card) {
                problems.push(`${file}: shows ${href}, which is not a moving card of the manual (media/NAME.webm)`);
            } else if (!['webm', 'png'].every((kind) => fs.existsSync(path.join(manual, 'media', `${card[1]}.${kind}`)))) {
                problems.push(`${file}: shows ${href}, but media/ has no ${card[1]}.webm and ${card[1]}.png`);
            }
            continue;
        }
        const [, target, anchor] = href.match(/^(?:(?:.*\/)?([\w-]+)\.md)?(?:#(.+))?$/) || [];
        const page = pages.get(target || path.basename(file, '.md'));
        if (!page) {
            problems.push(`${file}: links to ${href}, which is not a page of the manual`);
        } else if (anchor && !page.anchors.has(anchor)) {
            problems.push(`${file}: links to ${href}, but ${page.file} has no such heading`);
        }
    }
}

for (const { file, text } of pages.values()) {
    checkLinks(file, text);
}

for (const link of outside) {
    const [, target, anchor] = /^([\w-]+)(?:#(.+))?$/.exec(link) || [];
    const page = pages.get(target);
    if (!page) {
        problems.push(`${link} is not a page of the manual`);
    } else if (anchor && !page.anchors.has(anchor)) {
        problems.push(`${link}: ${page.file} has no such heading`);
    }
}

// The headings below the title, as many as the page has
const headings = (text) => [...text.matchAll(/^#{2,6} (.+)$/gm)].length;

// 07-debrand.sh (debrand.py) renames Universal Blue on every line but those that carry a
// copyright or licence mark, which is how upstream is credited. A line in the source that names them
// without one would come out of the build crediting Amethystora, or saying nothing at all, in any
// language. Run on the installed manual this finds nothing: the build checks the source pages too.
const UPSTREAM = /ublue|universal.?blue/i;
const CREDIT = /copyright|\(c\)\s|©|spdx-license|licen[cs]ed under|all rights reserved/i;
function checkCredits(file, raw) {
    raw.split('\n').forEach((line, at) => {
        if (UPSTREAM.test(line) && !CREDIT.test(line)) {
            problems.push(`${file}:${at + 1}: names upstream without a copyright mark, so the build would rename it: ${line.trim()}`);
        }
    });
}
for (const { file } of pages.values()) {
    checkCredits(file, fs.readFileSync(path.join(manual, file), 'utf8'));
}
const warnings = [];
let translated = 0;
for (const language of fs.readdirSync(manual).filter((name) => /^[a-z]{2,3}(-[A-Za-z0-9]{2,8})*$/.test(name))) {
    const folder = path.join(manual, language);
    if (!fs.statSync(folder).isDirectory()) {
        continue;
    }
    for (const name of fs.readdirSync(folder).filter((file) => file.endsWith('.md'))) {
        const file = `${language}/${name}`;
        const english = pages.get(path.basename(name, '.md'));
        if (!english) {
            problems.push(`${file}: translates no page of the manual`);
            continue;
        }
        const raw = fs.readFileSync(path.join(folder, name), 'utf8');
        checkCredits(file, raw);
        const text = raw.replace(/^```[\s\S]*?^```/gm, '');
        if (!/^# .+/m.test(text)) {
            problems.push(`${file}: no "# " title`);
        }
        if (headings(text) !== headings(english.text)) {
            warnings.push(`${file} has ${headings(text)} headings where ${english.file} has ${headings(english.text)}, so links into its sections open the English page`);
        }
        checkLinks(file, text);
        translated += 1;
    }
}

for (const warning of warnings) {
    console.log(`::warning::manual: ${warning}`);
}
for (const problem of problems) {
    console.error(`::error::manual: ${problem}`);
}
console.log(`manual: ${pages.size} pages and ${translated} translated pages checked${outside.length ? `, and ${outside.length} links from outside it` : ''}`);
process.exit(problems.length ? 1 : 0);
