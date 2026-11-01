// Today in the calendar menu: under GNOME's calendar and events, the tasks due today and the overdue ones from Notes,
// while Notes is open and unlocked (or not encrypted), and then GNOME's world clocks and the weather. The weather is
// off until the person chooses it, since it sends the place chosen in Weather to a weather service: until then its
// place in the menu says so and offers to turn it on, and nothing is fetched for it. Turned on, the weather also shows
// beside the clock in the top bar. For an account's first four weeks, Getting set comes first in the column
// (modules/today/getting.js).
//
// Three keys of org.gnome.shell.extensions.amethystora-ui.today (ame desktop today|weather|getting-set, Control), read
// live. Nothing runs until the menu opens: Notes is asked then, and listened to while the menu is open, and Getting
// set's lines are read then. The only timer is the weather's hourly refresh, which exists while the weather is on.
//
// For the tests only (tests/vm/suites/19-today.sh): taskTitles() is what the tasks section shows, refresh() asks Notes
// again.

import Clutter from 'gi://Clutter';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {GettingSet} from './today/getting.js';
import {Tasks} from './today/tasks.js';
import {Weather} from './today/weather.js';

const SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.today';
const CLASS = 'amethystora-today';

export class Today {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        const dateMenu = Main.panel.statusArea.dateMenu;
        if (!dateMenu)
            return;
        this._dateMenu = dateMenu;
        this._settings = this._ext.getSettings(SCHEMA);
        // The menu drawn as the board has it (stylesheets/today.css and the theme's shell-today.css)
        dateMenu.menu.box.add_style_class_name(CLASS);
        this._restyleToday(true);

        this._tasks = new Tasks(dateMenu, this._settings);
        this._tasks.enable();
        this._weather = new Weather(dateMenu, this._settings);
        this._weather.enable();
        // Getting set, first in the column (modules/today/getting.js)
        this._getting = new GettingSet(dateMenu, this._settings);
        this._getting.enable();
    }

    disable() {
        this._getting?.disable();
        this._getting = null;
        this._weather?.disable();
        this._weather = null;
        this._tasks?.disable();
        this._tasks = null;
        if (this._dateMenu) {
            this._restyleToday(false);
            this._dateMenu.menu.box.remove_style_class_name(CLASS);
        }
        this._dateMenu = null;
        // Disposed now rather than by the collector (dock.js says why)
        this._settings?.run_dispose();
        this._settings = null;
    }

    // GNOME's Today button: the date as the heading, the weekday on its end side. Its two labels are GNOME's own, only
    // laid out in a row; put back as they were.
    _restyleToday(on) {
        const button = this._dateMenu._date;
        const box = button?.get_child?.();
        const day = button?._dayLabel;
        const date = button?._dateLabel;
        if (!box || !day || !date || day.get_parent() !== box || date.get_parent() !== box)
            return;
        if (on) {
            box.orientation = Clutter.Orientation.HORIZONTAL;
            box.set_child_above_sibling(day, date);
            date.x_expand = true;
            day.add_style_class_name('amethystora-today-weekday');
            date.add_style_class_name('amethystora-today-date');
        } else {
            box.orientation = Clutter.Orientation.VERTICAL;
            box.set_child_below_sibling(day, date);
            date.x_expand = false;
            day.remove_style_class_name('amethystora-today-weekday');
            date.remove_style_class_name('amethystora-today-date');
        }
    }

    // ---- For the tests ----

    taskTitles() {
        return this._tasks?.titles() ?? [];
    }

    refresh() {
        this._tasks?.ask();
    }

    weatherCard() {
        return this._weather?.cardShown() ?? false;
    }

    weatherPill() {
        return this._weather?.pillShown() ?? false;
    }

    get getting() {
        return this._getting;
    }
}
