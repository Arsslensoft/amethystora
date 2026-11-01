// What the tiling changes in GNOME Shell's own windows and views while it runs: the window menu offers to float a
// window; the overview and GNOME's workspace-switch animation draw each window where the strip draws it (its
// frame and the translation that makes up the rest), and leave out the ones scrolled off the screen (and, when the
// settings say so, the floating windows or all but them); and the window switcher (Alt+Tab) shows its thumbnails
// larger. Each is a wrapper around GNOME's own method, taken away when the tiling stops.

import Clutter from 'gi://Clutter';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as AltTab from 'resource:///org/gnome/shell/ui/altTab.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';
import * as WindowMenu from 'resource:///org/gnome/shell/ui/windowMenu.js';
import * as WindowPreview from 'resource:///org/gnome/shell/ui/windowPreview.js';
import * as Workspace from 'resource:///org/gnome/shell/ui/workspace.js';
import * as WorkspaceThumbnail from 'resource:///org/gnome/shell/ui/workspaceThumbnail.js';
import * as WorkspaceAnimation from 'resource:///org/gnome/shell/ui/workspaceAnimation.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {config} from './config.js';

// GNOME's size for the app icon over a thumbnail, and the switcher's modes (altTab.js keeps them to itself)
const SWITCHER_APP_ICON = 48;
const APP_ICON_ONLY = 2;
const BOTH = 3;

export class ShellPatches {
    constructor(tiling) {
        this._tiling = tiling;
        this._undo = [];
    }

    enable() {
        const tiling = this._tiling;
        const engine = () => tiling.engine;

        // The window menu: one more entry at its top
        this._wrap(WindowMenu.WindowMenu.prototype, '_buildMenu', (original, menu, window) => {
            original.call(menu, window);
            // Every window's, a dialog's too: it then joins the floating layer
            const e = engine();
            if (!e)
                return;
            const item = new PopupMenu.PopupMenuItem(_('Float above the strip'));
            item.setOrnament(e.floating.has(window) ? PopupMenu.Ornament.CHECK : PopupMenu.Ornament.NONE);
            item.connect('activate', () => tiling.actions?.toggleFloating(window));
            menu.addMenuItem(item, 0);
        });

        // The overview shows only the floating windows, or leaves them out, when the settings say so
        const shown = window => {
            const floating = engine()?.floating?.has(window) ?? false;
            return config.overviewFloating === 'only' ? floating : config.overviewFloating !== 'none' || !floating;
        };
        this._wrap(Workspace.Workspace.prototype, '_isOverviewWindow', (original, workspace, window) =>
            original.call(workspace, window) && shown(window));
        this._wrap(WorkspaceThumbnail.WorkspaceThumbnail.prototype, '_isOverviewWindow', (original, thumbnail, actor) =>
            original.call(thumbnail, actor) && shown(actor.get_meta_window()));

        // The overview and the workspace switch see a window where it is drawn
        const visual = window => {
            const info = engine()?.info(window);
            return info?.strip ? info : null;
        };
        const proto = WindowPreview.WindowPreview.prototype;
        for (const name of ['boundingBox', 'windowCenter']) {
            const descriptor = Object.getOwnPropertyDescriptor(proto, name);
            if (!descriptor?.get)
                continue;
            Object.defineProperty(proto, name, {
                ...descriptor,
                get() {
                    const value = descriptor.get.call(this);
                    const actor = this.metaWindow?.get_compositor_private();
                    if (actor && visual(this.metaWindow)) {
                        value.x += Math.round(actor.translation_x);
                        value.y += Math.round(actor.translation_y);
                    }
                    return value;
                },
            });
            this._undo.push(() => Object.defineProperty(proto, name, descriptor));
        }
        const group = WorkspaceAnimation.BaseWorkspaceGroup?.prototype;
        if (group) {
            this._wrap(group, '_createClone', (original, workspaceGroup, windowActor) => {
                const clone = original.call(workspaceGroup, windowActor);
                if (visual(windowActor.meta_window)) {
                    clone.translation_x = windowActor.translation_x;
                    clone.translation_y = windowActor.translation_y;
                }
                return clone;
            });
            this._wrap(group, '_syncCloneVisibility', (original, workspaceGroup, clone) => {
                original.call(workspaceGroup, clone);
                if (visual(clone.source?.meta_window)?.hidden)
                    clone.visible = false;
            });
        }

        // The focus box gives way while GNOME slides from one workspace to the next. A slide that another cuts short
        // never calls its completion, but both end in the same method of GNOME's, so the box comes back there:
        // counted once when a slide begins, and settled once however it ends.
        const animation = Main.wm._workspaceAnimation;
        if (animation) {
            let hiding = null;
            this._wrap(animation, 'animateSwitch', (original, controller, from, to, direction, onComplete) => {
                const e = engine();
                // With a workspace on each screen, one that another screen shows already is there to see: the
                // switch is at once. To one shown nowhere GNOME slides, every screen, as it does without the tiling.
                if (e?.screens?.separate &&
                    e.screens.wasShown(global.workspace_manager.get_workspace_by_index(to)))
                    return onComplete();
                // With animations off GNOME's slide ends within its loop over the screens, as soon as it reaches the
                // primary one, and destroys what the rest of the loop goes on to move: nothing to slide, so no slide
                if (!St.Settings.get().enable_animations)
                    return onComplete();
                if (e && !hiding) {
                    hiding = e;
                    e.boxHidden++;
                }
                e?.strips().forEach(strip => e.updateBox(strip));
                return original.call(controller, from, to, direction, onComplete);
            });
            this._wrap(animation, '_finishWorkspaceSwitch', (original, controller, ...args) => {
                try {
                    return original.call(controller, ...args);
                } finally {
                    const e = hiding;
                    hiding = null;
                    if (e?.strips && tiling.engine === e) {
                        e.boxHidden--;
                        for (const strip of e.strips())
                            e.updateBox(strip);
                    }
                }
            });
        }

        // GNOME's screenshot tool takes windows whole, edges cut at the screen included
        const screenshot = Main.screenshotUI;
        if (screenshot) {
            screenshot.connectObject('notify::visible', () => engine()?.setUnclipped(screenshot.visible), this);
            this._undo.push(() => {
                screenshot.disconnectObject(this);
                engine()?.setUnclipped(false);
            });
        }

        // The window switcher's thumbnails, larger than GNOME's
        if (AltTab.WindowIcon) {
            this._wrap(AltTab.WindowIcon.prototype, '_init', (original, icon, window, mode) => {
                original.call(icon, window, mode);
                if (mode === APP_ICON_ONLY)
                    return;
                const actor = window.get_compositor_private();
                const monitor = Main.layoutManager.monitors[global.display.get_current_monitor()];
                if (!actor || !monitor)
                    return;
                const scale = St.ThemeContext.get_for_stage(global.stage).scale_factor;
                const size = Math.round(monitor.height * config.switcherShare);
                const [width, height] = actor.get_size();
                const fit = Math.min(1, size * scale / width, size * scale / height);
                icon._icon.destroy_all_children();
                icon._icon.add_child(new Clutter.Clone({
                    source: actor, width: width * fit, height: height * fit,
                    x_align: Clutter.ActorAlign.CENTER, y_align: Clutter.ActorAlign.CENTER,
                    x_expand: true, y_expand: true,
                }));
                if (mode === BOTH && icon.app)
                    icon._icon.add_child(icon._createAppIcon(icon.app, SWITCHER_APP_ICON));
                icon._icon.set_size(size * scale, size * scale);
            });
        }
    }

    disable() {
        for (const undo of this._undo.splice(0).reverse())
            undo();
    }

    // Puts a wrapper in place of a GNOME Shell method, called as wrapper(original, object, ...arguments), and the
    // original back on disable
    _wrap(object, name, wrapper) {
        const original = object[name];
        if (typeof original !== 'function')
            return;
        const own = Object.prototype.hasOwnProperty.call(object, name);
        object[name] = function (...args) {
            return wrapper(original, this, ...args);
        };
        this._undo.push(() => {
            if (own)
                object[name] = original;
            else
                delete object[name];
        });
    }
}
