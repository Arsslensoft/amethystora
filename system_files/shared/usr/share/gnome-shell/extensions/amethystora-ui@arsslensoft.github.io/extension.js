// Amethystora's own desktop, in place of the third-party extensions the image used to ship. Each part is a module
// in modules/, written for this image from what those extensions did with the image's settings and nothing more
// (plans/PLAN-amethystora-ui.md). Every module is started on its own, so one that fails leaves the others running
// and says why in the journal. A module's stylesheet, stylesheets/<module>.css, is loaded while it runs; the
// colours that follow the theme are in /usr/share/amethystora/themed/shell-<module>.css.tpl (modules/theme.js).

import GLib from 'gi://GLib';

import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

// Before anything else makes a settings object
import './modules/keepsettings.js';
import {batchStylesheets, replaceStylesheets} from './modules/theme/stylesheets.js';
import {Theme} from './modules/theme.js';
import {Style} from './modules/style.js';
import {Comfort} from './modules/comfort.js';
import {Tweaks} from './modules/tweaks.js';
import {Workspaces} from './modules/workspaces.js';
import {Meters} from './modules/meters.js';
import {Tray} from './modules/tray.js';
import {Search} from './modules/search.js';
import {Emoji} from './modules/emoji.js';
import {Typing} from './modules/typing.js';
import {Keys} from './modules/keys.js';
import {Fuzzy} from './modules/fuzzy.js';
import {Screenshot} from './modules/screenshot.js';
import {Bazaar} from './modules/bazaar.js';
import {Connect} from './modules/connect.js';
import {BackupDisk} from './modules/backupdisk.js';
import {Pill} from './modules/pill.js';
import {Quick} from './modules/quick.js';
import {Bars} from './modules/bars.js';
import {Today} from './modules/today.js';
import {Power} from './modules/power.js';
import {Audio} from './modules/audio.js';
import {Brightness} from './modules/brightness.js';
import {Dock} from './modules/dock.js';
import {Glass} from './modules/glass.js';
import {Hints} from './modules/hints.js';
import {FirstLook} from './modules/firstlook.js';
import {Hello} from './modules/hello.js';
import {AppGrid} from './modules/appgrid.js';
import {Pointer} from './modules/pointer.js';
import {Tiling} from './modules/tiling.js';
import {Windows} from './modules/windows.js';
import {Motion} from './modules/motion.js';
import {Sounds} from './modules/sounds.js';
import {Bezel} from './modules/bezel.js';
import {Captions} from './modules/captions.js';
import {Access} from './modules/access.js';
import {Touch} from './modules/touch.js';
import {Present} from './modules/present.js';
import {DeskWidgets} from './modules/deskwidgets.js';
import {Shelf} from './modules/shelf.js';
import {Desks} from './modules/desks.js';
import {Agent} from './modules/agent.js';
import {Quit} from './modules/quit.js';
import {Privacy} from './modules/privacy.js';
import {Tools} from './modules/tools.js';
import {Guide} from './modules/guide.js';
import {Rest} from './modules/rest.js';
import {Living} from './modules/living.js';
import {Plugins} from './modules/plugins.js';
import {Found} from './modules/found.js';
import {Focus} from './modules/focus.js';

// In the order they start, by the name of their stylesheet; they stop in the opposite order. The desktop style comes
// first after the theme, since the glass asks it; the glass comes after the dock and the panel's modules, whose actors
// it draws behind. Plugins come right after the pill, whose extra place and notices they use, and before the modules that ask them for
// what plugins show (the desktop's cards). Present mode comes after the pill, whose hold it asks. The agent comes after the pill, where it
// shows its changes, and before force quit, whose dialog asks it. The resting screen and the living wallpapers come
// after comfort, the pill and present mode, which they ask. Focus profiles come last: a profile lays itself over what
// the others do, and stops first.
const MODULES = [
    ['theme', Theme],
    ['style', Style],
    ['comfort', Comfort],
    ['tweaks', Tweaks],
    ['workspaces', Workspaces],
    ['meters', Meters],
    ['tray', Tray],
    ['search', Search],
    ['fuzzy', Fuzzy],
    ['screenshot', Screenshot],
    ['bazaar', Bazaar],
    ['connect', Connect],
    ['backupdisk', BackupDisk],
    ['pill', Pill],
    ['plugins', Plugins],
    ['quick', Quick],
    ['bars', Bars],
    ['today', Today],
    ['power', Power],
    ['audio', Audio],
    ['brightness', Brightness],
    ['dock', Dock],
    ['appgrid', AppGrid],
    ['emoji', Emoji],
    ['typing', Typing],
    ['keys', Keys],
    ['glass', Glass],
    ['hints', Hints],
    ['firstlook', FirstLook],
    ['hello', Hello],
    ['pointer', Pointer],
    ['tiling', Tiling],
    ['windows', Windows],
    ['motion', Motion],
    ['sounds', Sounds],
    ['bezel', Bezel],
    ['captions', Captions],
    ['access', Access],
    ['touch', Touch],
    ['present', Present],
    ['deskwidgets', DeskWidgets],
    ['shelf', Shelf],
    ['desks', Desks],
    ['agent', Agent],
    ['quit', Quit],
    ['privacy', Privacy],
    ['tools', Tools],
    ['guide', Guide],
    ['rest', Rest],
    ['living', Living],
    ['found', Found],
    ['focus', Focus],
];

// A pump tick enables modules until it has spent this long, then hands the main loop back for a frame
const PUMP_BUDGET_US = 8000;

export default class AmethystoraUI extends Extension {
    enable() {
        // GNOME turns an extension on and off asynchronously: modules still running from an enable that no disable
        // reached are stopped first, or they would run on, unseen, beside the new ones (their laters included)
        if (this.modules)
            this.disable();
        // The running modules by name, for a module that needs another's actors (the glass, the dock's)
        this.modules = new Map();
        this._queue = MODULES.slice();
        // One rebuild of the shell's theme for every stylesheet at once (stylesheets.js): the static module sheets
        // here and the themed shell-*.css the theme module loads, together. The modules then start already styled,
        // so none of the ones started below adds a restyle of its own.
        batchStylesheets(() => {
            const stylesheets = [];
            for (const [name] of MODULES) {
                const file = this.dir.get_child('stylesheets').get_child(`${name}.css`);
                if (file.query_exists(null))
                    stylesheets.push([name, file]);
            }
            this.stylesheets = replaceStylesheets([], stylesheets).map(([, file]) => file);
            // The theme module inside the batch, so its themed sheets join the one restyle at the batch's end
            this._enableNext();
        });
        // The rest start a few at a time, below the redraw's priority, so the main loop paints between ticks:
        // starting all of them in one go froze it for most of a second at every login and unlock, which stuttered
        // the login reveal and the unlock. A module still sees only the modules before it in the list, enabled or
        // not, as it did when they all started together.
        this._schedulePump();
    }

    // Enables the next module in the list, in order. Returns false once the queue is empty.
    _enableNext() {
        const entry = this._queue?.shift();
        if (!entry)
            return false;
        const [name, Module] = entry;
        const module = new Module(this);
        try {
            module.enable();
            this.modules.set(name, module);
        } catch (e) {
            console.error(`Amethystora UI: the ${name} module did not start: ${e.message}\n${e.stack}`);
            // A module stops cleanly from wherever it got to
            try {
                module.disable();
            } catch {}
        }
        return true;
    }

    _schedulePump() {
        if (this._pumpId || !this._queue?.length)
            return;
        // Below the redraw's priority, so the shell paints between ticks and resolves the actors each tick added
        this._pumpId = GLib.idle_add(GLib.PRIORITY_LOW, () => {
            this._pumpId = 0;
            this._pump();
            return GLib.SOURCE_REMOVE;
        });
    }

    // As many modules as fit in a short budget, then back to the main loop for a frame
    _pump() {
        const started = GLib.get_monotonic_time();
        while (this._enableNext()) {
            if (this._queue.length && GLib.get_monotonic_time() - started > PUMP_BUDGET_US) {
                this._schedulePump();
                return;
            }
        }
    }

    disable() {
        // Nothing left of a start that had not finished: the pump, and its queue
        if (this._pumpId) {
            GLib.source_remove(this._pumpId);
            this._pumpId = 0;
        }
        this._queue = null;
        batchStylesheets(() => this._disable());
    }

    _disable() {
        // Before anything stops, while every module still runs: what a module keeps of the desktop as it is (a lock
        // keeping the open apps for an update's restart, workspaces/resume.js)
        for (const [name, module] of this.modules ?? []) {
            try {
                module.beforeDisable?.();
            } catch (e) {
                console.error(`Amethystora UI: the ${name} module could not keep its state: ${e.message}`);
            }
        }
        for (const [name, module] of [...this.modules ?? []].reverse()) {
            try {
                module.disable();
            } catch (e) {
                console.error(`Amethystora UI: the ${name} module did not stop cleanly: ${e.message}`);
            }
        }
        if (this.stylesheets?.length)
            replaceStylesheets(this.stylesheets, []);
        this.modules = this.stylesheets = null;
    }
}
