// Amethystora's own desktop, in place of the third-party extensions the image used to ship. Each part is a module
// in modules/, written for this image from what those extensions did with the image's settings and nothing more
// (plans/PLAN-amethystora-ui.md). Every module is started on its own, so one that fails leaves the others running
// and says why in the journal. A module's stylesheet, stylesheets/<module>.css, is loaded while it runs; the
// colours that follow the theme are in /usr/share/amethystora/themed/shell-<module>.css.tpl (modules/theme.js).

import St from 'gi://St';

import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

// Before anything else makes a settings object
import './modules/keepsettings.js';
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
import {Focus} from './modules/focus.js';

// In the order they start, by the name of their stylesheet; they stop in the opposite order. The desktop style comes
// first after the theme, since the glass asks it; the glass comes after the dock and the panel's modules, whose actors
// it draws behind. Present mode comes after the pill, whose hold it asks. The agent comes after the pill, where it
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
    ['focus', Focus],
];

export default class AmethystoraUI extends Extension {
    enable() {
        // GNOME turns an extension on and off asynchronously: modules still running from an enable that no disable
        // reached are stopped first, or they would run on, unseen, beside the new ones (their laters included)
        if (this.modules)
            this.disable();
        // The running modules by name, for a module that needs another's actors (the glass, the dock's)
        this.modules = new Map();
        for (const [name, Module] of MODULES) {
            const module = new Module(this);
            try {
                module.enable();
                this.modules.set(name, module);
                this._loadStylesheet(name, module);
            } catch (e) {
                console.error(`Amethystora UI: the ${name} module did not start: ${e.message}\n${e.stack}`);
                // A module stops cleanly from wherever it got to
                try {
                    module.disable();
                } catch {}
            }
        }
    }

    disable() {
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
            if (module.stylesheet)
                St.ThemeContext.get_for_stage(global.stage).get_theme().unload_stylesheet(module.stylesheet);
        }
        this.modules = null;
    }

    _loadStylesheet(name, module) {
        const file = this.dir.get_child('stylesheets').get_child(`${name}.css`);
        if (!file.query_exists(null))
            return;
        try {
            St.ThemeContext.get_for_stage(global.stage).get_theme().load_stylesheet(file);
            module.stylesheet = file;
        } catch (e) {
            console.error(`Amethystora UI: stylesheets/${name}.css: ${e.message}`);
        }
    }
}

