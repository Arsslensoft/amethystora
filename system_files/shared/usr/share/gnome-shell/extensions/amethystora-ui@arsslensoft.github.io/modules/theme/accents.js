// The twelve accents any theme can take, by id and fill, in the order Control shows them: the same table as ACCENTS in
// /usr/lib/amethystora/theme/lib.sh, which 20-tests.sh holds this one to. The names are said by the workspaces' menu.

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

export const ACCENTS = [
    ['amethyst', '#9148f0'],
    ['iris', '#5b5cf0'],
    ['sky', '#2172e2'],
    ['lagoon', '#008194'],
    ['jade', '#16865c'],
    ['moss', '#5b7f1f'],
    ['amber', '#a66800'],
    ['ember', '#c6531c'],
    ['coral', '#d63b5c'],
    ['rose', '#c2338f'],
    ['orchid', '#a43fc7'],
    ['slate', '#5b6680'],
];

export const IDS = ACCENTS.map(([id]) => id);

// The schema of the theme module, whose keys say each workspace's accent
export const SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.theme';

// An accent's name as a person reads it; called only from a method, where gettext is ready
export function accentName(id) {
    return {
        amethyst: _('Amethyst'),
        iris: _('Iris'),
        sky: _('Sky'),
        lagoon: _('Lagoon'),
        jade: _('Jade'),
        moss: _('Moss'),
        amber: _('Amber'),
        ember: _('Ember'),
        coral: _('Coral'),
        rose: _('Rose'),
        orchid: _('Orchid'),
        slate: _('Slate'),
    }[id] ?? id;
}

// The accent a workspace takes, by its place: one of the twelve, or null for the desktop's own (a workspace past the
// list, or anything else in it)
export function workspaceAccent(settings, index) {
    const id = settings.get_strv('workspace-accents')[index];
    return IDS.includes(id) ? id : null;
}
