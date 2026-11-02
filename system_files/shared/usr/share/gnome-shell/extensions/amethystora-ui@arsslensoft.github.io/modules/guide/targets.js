// What a guide's step may point at in the shell: a name for an actor. `via` is what to point at while the actor is not on
// the screen (the toggle inside a closed menu). A guide can name nothing else, and check-guides.mjs holds it to this table.

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

const LOGO_MENU = 'amethystora-logo-menu@arsslensoft.github.io';

// Quick settings' own, which GNOME keeps in fields of its menu: a field it renames only loses the ring, and the step says
// its words (panel.js, status/bluetooth.js and status/network.js, the same in GNOME 50 and 51)
const quick = () => Main.panel.statusArea.quickSettings ?? null;

export const TARGETS = {
    'quick-settings': {find: quick},
    'quick-bluetooth': {via: 'quick-settings', find: () => quick()?._bluetooth?.quickSettingsItems?.[0] ?? null},
    'quick-wifi': {via: 'quick-settings', find: () => quick()?._network?._wirelessToggle ?? null},
    'logo-menu': {find: () => Main.panel.statusArea[LOGO_MENU] ?? null},
};
