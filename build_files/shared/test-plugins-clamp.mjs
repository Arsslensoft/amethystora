// What the shell does with what a plugin printed (plugins/clamp.js): the host cleans it, and the shell, being the compositor, clamps it
// again and draws nothing but text. Run by 20-tests.sh, against the file itself.
import assert from 'node:assert/strict';
import {fileURLToPath} from 'node:url';
import path from 'node:path';

const root = process.argv[2] ?? path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const clamp = await import(path.join(root, 'system_files/shared/usr/share/gnome-shell/extensions/amethystora-ui@arsslensoft.github.io/modules/plugins/clamp.js'));

// Text: no markup is read (it is only ever drawn as text), no control or bidi characters, bounded
assert.equal(clamp.line('a‮b\u0007c\nd', 20), 'a b c d');
assert.equal(clamp.line('x'.repeat(500), 10).length, 10);
assert.equal(clamp.line(42), '');
assert.equal(clamp.line('<b>bold</b>'), '<b>bold</b>', 'markup stays text');

// Icons are theme names, never a path
assert.equal(clamp.icon('weather-clear-symbolic'), 'weather-clear-symbolic');
for (const bad of ['/etc/passwd', '../x', '.hidden', 'a/b', 'file:///x', 'x y', '', 'a'.repeat(65), 7]) assert.equal(clamp.icon(bad), '', String(bad));

// An output: a card of at most twenty, numbers inside their range, an action only from the manifest
const out = clamp.output({
    text: 'T\u0000', icon: '/x', class: 'system', progress: 7, until: 1e15, hidden: 1, private: true,
    menu: [{label: 'Open', action: 'open-it'}, {separator: true}, {label: 'No', action: 'rm-rf'}, 'x', null],
    card: Array.from({length: 40}, (_, i) => ({type: 'text', text: `row ${i}`})).concat([{type: 'frame', text: 'x'}, {type: 'meter', min: 5, max: 1, value: 3}]),
}, ['open-it']);
assert.equal(out.text, 'T');
assert.equal(out.icon, '');
assert.equal(out.class, '', 'no class but the four');
assert.equal(out.progress, 1);
assert.equal(out.until, null);
assert.equal(out.hidden, false);
assert.equal(out.private, true);
assert.equal(out.card.length, 20);
assert.deepEqual(out.menu.map(i => i.action ?? null), ['open-it', null, null, null].slice(0, 3));
assert.ok(!out.card.some(e => e.type === 'frame'), 'an element outside the set is dropped');
assert.equal(clamp.output('text'), null);
assert.equal(clamp.element({type: 'meter', min: 5, max: 1, value: 3}), null);
assert.deepEqual(clamp.element({type: 'meter', min: 0, max: 10, value: 99}), {type: 'meter', label: '', value: 10, min: 0, max: 10});
assert.equal(clamp.element({type: 'switch', label: 'x', on: 'yes', action: 'go'}, ['go']).on, false);

// A plugin as the host lists it: ids are reverse-domain names, places carry only what the shell reads, and nothing carries a plate
const p = clamp.plugin({
    id: 'dev.example.ci', name: 'CI', version: '1.0.0', places: {pill: {plate: 'link'}, bar: {position: 'under'}, search: {prefix: 'ci:'}, dock: {app: '../x.desktop'}, lock: {}},
    paused: true, reason: 'sudo', plate: 'call', output: {text: 'ok'}, granted: ['notify', 7],
});
assert.deepEqual(p.places, {pill: {}, bar: {position: 'right', order: 0}, search: {prefix: 'ci:'}});
assert.equal(p.reason, '');
assert.equal('plate' in p, false);
assert.deepEqual(p.granted, ['notify']);
assert.equal(clamp.plugin({id: 'amethystora'}), null);
assert.equal(clamp.plugin({id: 'A.B'}), null);
assert.equal(clamp.plugin({id: '../x.y'}), null);
assert.equal(clamp.plugin(null), null);
assert.deepEqual(clamp.plugin({id: 'dev.example.d', places: {dock: {app: 'org.gnome.Nautilus.desktop'}}}).places, {dock: {app: 'org.gnome.Nautilus.desktop'}});

// A script's pill: text, a number, a time, and its owner
const s = clamp.script({name: 'Backup', text: 'Copying‮', progress: -3, until: 'soon', icon: 'x/y', owner: 1234, started: 1});
assert.deepEqual([s.name, s.text, s.progress, s.until, s.icon, s.owner], ['Backup', 'Copying', 0, null, '', 1234]);
assert.equal(clamp.script({text: 'no name'}), null);
assert.equal(clamp.script([]), null);

// What a permission says
assert.deepEqual(clamp.reach(['network=api.github.com', 'notify']), [{kind: 'network', what: 'api.github.com'}, {kind: 'notify', what: ''}]);
console.log('plugins clamp: ok');
