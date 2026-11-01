// `amethystora-widgets run <id>`: run a widget once the way the top bar does, print what the bar would
// show, and say what is wrong with it. Exits 1 on anything the bar would show as a warning sign.
//
//   gjs -m check.js <widget folder>

import GLib from 'gi://GLib';
import Gio from 'gi://Gio';
import System from 'system';

import {PATH, Runner, TIME_LIMIT, commandArgv, loadWidget} from './runner.js';

// Seconds to wait for the first update of a command that keeps running
const FIRST_UPDATE = 10;

const errors = [];
const warnings = [];

// What a command names has to be there, as the bar's PATH finds it
function checkCommand(widget, where, command) {
    let argv;
    try {
        argv = commandArgv(widget, command);
    } catch (e) {
        errors.push(`${where}: ${e.message}`);
        return;
    }
    const program = argv[0];
    if (program.includes('/')) {
        if (!GLib.file_test(program, GLib.FileTest.EXISTS))
            errors.push(`${where}: ${program} does not exist`);
        else if (!GLib.file_test(program, GLib.FileTest.IS_EXECUTABLE))
            errors.push(`${where}: ${program} is not executable; chmod +x it`);
    } else if (!GLib.find_program_in_path(program)) {
        errors.push(`${where}: ${program} is not on the PATH the bar uses (${PATH})`);
    }
}

// The bar loads a widget again whenever its folder changes, so a command that writes there runs over
// and over
function snapshot(dir) {
    const files = new Map();
    const children = Gio.File.new_for_path(dir).enumerate_children(
        'standard::name,time::modified,time::modified-usec', Gio.FileQueryInfoFlags.NOFOLLOW_SYMLINKS, null);
    let info;
    while ((info = children.next_file(null))) {
        files.set(info.get_name(),
            `${info.get_attribute_uint64('time::modified')}.${info.get_attribute_uint32('time::modified-usec')}`);
    }
    return files;
}

function finish() {
    for (const warning of warnings)
        printerr(`warning: ${warning}`);
    for (const error of errors)
        printerr(`error: ${error}`);
    if (!errors.length)
        print(warnings.length ? 'It works, with the warnings above.' : 'It works.');
    System.exit(errors.length ? 1 : 0);
}

GLib.setenv('PATH', PATH, true);
const widget = loadWidget(System.programArgs[0] ?? '.');
warnings.push(...widget.warnings);
errors.push(...widget.errors);
if (errors.length)
    finish();
checkCommand(widget, '"command"', widget.manifest.command);
if (errors.length)
    finish();

const loop = new GLib.MainLoop(null, false);
const before = snapshot(widget.dir);
let state = null;
const runner = new Runner(widget,
    (update, problems) => {
        state = update;
        warnings.push(...problems);
        loop.quit();
    },
    message => {
        errors.push(message);
        loop.quit();
    });
if (widget.manifest.interval === 0) {
    GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, FIRST_UPDATE, () => {
        errors.push(`it printed nothing in ${FIRST_UPDATE} seconds; a command that keeps running has to ` +
            'print what to show straight away, and then a line each time it changes');
        loop.quit();
        return GLib.SOURCE_REMOVE;
    });
} else {
    print(`Running it, for up to ${TIME_LIMIT} seconds...`);
}
runner.start();
loop.run();
runner.stop();

const after = snapshot(widget.dir);
const changed = [...new Set([...before.keys(), ...after.keys()])].filter(name => before.get(name) !== after.get(name));
if (changed.length) {
    errors.push(`it changed ${changed.join(', ')} in its own folder, which makes the bar load it again, ` +
        'and run it again, without end; keep what it writes in $WIDGET_STATE');
}
if (widget.manifest.place === 'desktop') {
    warnings.push('it is drawn on the desktop only while widgets on the desktop are on ' +
        '(ame desktop desktop-widgets on), never in the bar');
}
if (state) {
    if (state.click)
        checkCommand(widget, '"click"', state.click);
    state.menu.forEach((item, i) => {
        if (item.run)
            checkCommand(widget, `"run" of menu item ${i + 1}`, item.run);
    });
    print(`What the bar shows:\n${JSON.stringify(state, null, 2)}`);
}
finish();
