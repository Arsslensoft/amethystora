// What stands behind whole screens. The overview, and the slide from one workspace to another, sit on the wallpaper
// pushed far back behind a heavier, darker frost; in the screenshot UI's window mode the windows are laid out on a
// pane of glass the size of the screen. An app folder opens over the app grid blurred live, the blur growing as the
// folder zooms out of its icon and going as it zooms back, in place of GNOME's darkening. While the glass is solid all
// of these are GNOME's own: this part then rests, its wrapped methods doing nothing more than GNOME's.

import Clutter from 'gi://Clutter';
import Cogl from 'gi://Cogl';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';
import St from 'gi://St';

import * as AppDisplay from 'resource:///org/gnome/shell/ui/appDisplay.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as Screenshot from 'resource:///org/gnome/shell/ui/screenshot.js';

const FOLDER_BLUR = 'amethystora-glass-folder-blur';
const FOLDER = 'amethystora-glass-folder';
// The folder's blur, in pixels, and how long it takes to grow or go: as long as GNOME's zoom
const FOLDER_RADIUS = 48;
const FOLDER_TIME = 200;
const CLEAR = new Cogl.Color({red: 0, green: 0, blue: 0, alpha: 0});

// Every screen, in the coordinates of an actor that covers the whole stage
const screens = actor => Main.layoutManager.monitors.map(m => [m.x - actor.x, m.y - actor.y, m.width, m.height]);

export class Backdrops {
    constructor(glass) {
        this._glass = glass;
    }

    enable() {
        const glass = this._glass;
        this._folders = new Set();
        this._on = true;
        glass.attach(Main.layoutManager.overviewGroup, {material: 'frost', radius: 0, areas: screens, inClones: true},
            {classes: ['amethystora-glass-overview']});
        if (Main.screenshotUI)
            this._screenshot(Main.screenshotUI);

        // The slide between workspaces is a group per screen, added over the windows for as long as it runs
        Main.uiGroup.connectObject('child-added', (group, child) => {
            if (child.has_style_class_name?.('workspace-animation'))
                this._slide(child);
        }, this);

        // The methods are wrapped once for as long as the glass runs (it puts them back), and do their part only
        // while this part is on
        if (this._wrapped)
            return;
        this._wrapped = true;
        // The screenshot UI makes its window selectors once, and again whenever the screens change
        glass.wrap(Screenshot.ScreenshotUI.prototype, '_rebuildMonitorBins', (rebuild, ui, args) => {
            rebuild.apply(ui, args);
            if (this._on)
                this._screenshot(ui);
        });
        const proto = AppDisplay.AppFolderDialog.prototype;
        glass.wrap(proto, '_zoomAndFadeIn', (zoom, dialog, args) => {
            zoom.apply(dialog, args);
            if (this._on)
                this._folderOpening(dialog);
        });
        glass.wrap(proto, '_zoomAndFadeOut', (zoom, dialog, args) => {
            zoom.apply(dialog, args);
            this._folderClosing(dialog);
        });
        // GNOME lightens the shade while an icon is dragged out of the folder: there is none to lighten
        glass.wrap(proto, '_setLighterBackground', (set, dialog, args) => {
            set.apply(dialog, args);
            this._clearShade(dialog);
        });
    }

    disable() {
        this._on = false;
        Main.uiGroup.disconnectObject(this);
        for (const dialog of this._folders ?? [])
            this._folderDone(dialog);
        this._folders = null;
        this._glass.detach(Main.layoutManager.overviewGroup);
        for (const selector of Main.screenshotUI?._windowSelectors ?? [])
            this._glass.detach(selector);
    }

    _screenshot(ui) {
        for (const selector of ui._windowSelectors ?? []) {
            this._glass.attach(selector, {material: 'pane', radius: 16, inClones: true},
                {classes: ['amethystora-glass-backdrop']});
        }
    }

    // Frost behind the sliding workspaces, and glass behind the copies of the windows that have it
    _slide(group) {
        const glass = this._glass;
        glass.attach(group, {material: 'frost', radius: 0, inClones: true}, {classes: ['amethystora-glass-slide']});
        const look = glass.windows.cloneLook();
        const walk = actor => {
            for (const child of actor.get_children()) {
                if (child instanceof Clutter.Clone && child.source instanceof Meta.WindowActor) {
                    if (glass.has(child.source))
                        glass.attach(child, {...look, radius: glass.effectOf(child.source).look.radius});
                } else {
                    walk(child);
                }
            }
        };
        walk(group);
    }

    _clearShade(dialog) {
        if (!this._folders?.has(dialog))
            return;
        dialog.remove_transition('background-color');
        dialog.background_color = CLEAR;
    }

    _folderOpening(dialog) {
        this._folders.add(dialog);
        dialog.add_style_class_name(FOLDER);
        this._clearShade(dialog);
        if (!dialog.get_effect(FOLDER_BLUR))
            dialog.add_effect_with_name(FOLDER_BLUR, new Shell.BlurEffect({mode: Shell.BlurMode.BACKGROUND}));
        const radius = Math.round(FOLDER_RADIUS * St.ThemeContext.get_for_stage(global.stage).scale_factor);
        dialog.ease_property(`@effects.${FOLDER_BLUR}.radius`, radius, {
            duration: FOLDER_TIME,
            mode: Clutter.AnimationMode.EASE_OUT_QUAD,
        });
    }

    _folderClosing(dialog) {
        if (!this._folders?.has(dialog))
            return;
        this._clearShade(dialog);
        if (!dialog.visible) {
            this._folderDone(dialog);
            return;
        }
        dialog.ease_property(`@effects.${FOLDER_BLUR}.radius`, 0, {
            duration: FOLDER_TIME,
            mode: Clutter.AnimationMode.EASE_IN_QUAD,
            onStopped: finished => {
                if (finished)
                    this._folderDone(dialog);
            },
        });
    }

    // Nothing is kept once a folder has closed: the live blur costs a pass over the screen on every frame
    _folderDone(dialog) {
        this._folders?.delete(dialog);
        dialog.remove_transition(`@effects.${FOLDER_BLUR}.radius`);
        const blur = dialog.get_effect(FOLDER_BLUR);
        if (blur)
            dialog.remove_effect(blur);
        dialog.remove_style_class_name(FOLDER);
    }
}
