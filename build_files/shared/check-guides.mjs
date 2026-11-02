// Show me (R7-61): the guides, checked at build time like the manual's links. Every guide is beside a section that
// exists and the page links to it; every shell step names a target in targets.js; every app step names a launcher that
// exists and a widget by a message of that app's own catalog; and every guide: link in the manual lands on a guide.
//   node check-guides.mjs UI_EXTENSION MANUAL_DIR LIB_DIR APPLICATIONS_DIR

import fs from 'node:fs';
import path from 'node:path';
import {pathToFileURL} from 'node:url';

const [extension, manual, lib, applications] = process.argv.slice(2);
const problems = [];
const {guides} = await import(pathToFileURL(path.join(path.resolve(extension), 'modules/guide/guides.js')).href);
const list = guides(text => text);

// apps/common/src/markdown.rs's slug, which check-manual.js keeps too
const slug = text => {
    let out = '';
    let dash = false;
    for (const c of text.replace(/[&<>"']/g, '').toLowerCase()) {
        if (/[\p{L}\p{N}]/u.test(c)) {
            if (dash && out)
                out += '-';
            dash = false;
            out += c;
        } else {
            dash = true;
        }
    }
    return out;
};

const targets = [...fs.readFileSync(path.join(extension, 'modules/guide/targets.js'), 'utf8').matchAll(/^\s+'([a-z-]+)': \{/gm)].map(match => match[1]);
const via = [...fs.readFileSync(path.join(extension, 'modules/guide/targets.js'), 'utf8').matchAll(/via: '([a-z-]+)'/g)].map(match => match[1]);
for (const name of via) {
    if (!targets.includes(name))
        problems.push(`targets.js: via ${name}, which is not a target`);
}

const ids = new Set();
for (const guide of list) {
    if (!/^[a-z0-9-]{1,64}$/.test(guide.id))
        problems.push(`a guide's id: ${guide.id}`);
    if (ids.has(guide.id))
        problems.push(`two guides are ${guide.id}`);
    ids.add(guide.id);
    if (!guide.title || !guide.steps?.length)
        problems.push(`${guide.id} has no title or no steps`);
    const file = path.join(manual, `${guide.page}.md`);
    if (!fs.existsSync(file)) {
        problems.push(`${guide.id} is beside ${guide.page}, which is not a page of the manual`);
        continue;
    }
    const text = fs.readFileSync(file, 'utf8');
    const anchors = new Set([...text.matchAll(/^#{2,6} (.+)$/gm)].map(match => slug(match[1])));
    if (guide.section && !anchors.has(guide.section))
        problems.push(`${guide.id} is beside ${guide.page}#${guide.section}, which has no such heading`);
    if (!text.includes(`](guide:${guide.id})`))
        problems.push(`${guide.page} has no link to its guide ${guide.id}`);
    for (const step of guide.steps) {
        if (!step.say)
            problems.push(`${guide.id}: a step with no words`);
        if (step.shell && !targets.includes(step.shell))
            problems.push(`${guide.id}: ${step.shell} is not a target of targets.js`);
        if (step.app) {
            if (!fs.existsSync(path.join(applications, step.launcher ?? '')))
                problems.push(`${guide.id}: no launcher ${step.launcher}`);
            let catalog = {};
            try {
                catalog = JSON.parse(fs.readFileSync(path.join(lib, step.app, 'resources/app/locale/en.json'), 'utf8'));
            } catch (e) {
                problems.push(`${guide.id}: ${step.app} has no catalog: ${e.message}`);
            }
            if (!Object.hasOwn(catalog, step.find))
                problems.push(`${guide.id}: ${step.app} says no "${step.find}"`);
        }
    }
}

// Every guide: link in the manual's English pages lands on a guide
for (const name of fs.readdirSync(manual).filter(one => one.endsWith('.md'))) {
    for (const match of fs.readFileSync(path.join(manual, name), 'utf8').matchAll(/\]\(guide:([^)\s]*)\)/g)) {
        if (!ids.has(match[1]))
            problems.push(`${name} links to guide:${match[1]}, which is no guide`);
    }
}

if (problems.length) {
    console.error(problems.join('\n'));
    process.exit(1);
}
console.log(`${list.length} guides checked`);
