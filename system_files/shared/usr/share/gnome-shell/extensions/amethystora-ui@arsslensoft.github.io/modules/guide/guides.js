// Show me, on my screen (R7-61): the guides, each beside a section of the manual, whose page links to it as
// [Show me](guide:ID). A step says what to do and names what to point at: `shell`, a name in targets.js, or `app` with
// its launcher, and `find`, the English words on a widget, which the app looks for in the session's language
// (apps/common/src/where_is.rs). A step that names nothing is words only.
//
// A function of `_`, with nothing imported, so that build_files/shared/check-guides.mjs can read it as the shell does and
// hold every name here to something that exists, as check-manual.js does a page's links.

export const guides = _ => [
    {
        id: 'turn-off-bluetooth', title: _('Turn off Bluetooth'), page: 'hardware', section: 'turn-off-bluetooth',
        steps: [
            {say: _('Open this menu, here at the top right.'), shell: 'quick-settings'},
            {say: _('Press Bluetooth. It goes dark when it is off.'), shell: 'quick-bluetooth'},
        ],
    },
    {
        id: 'join-wifi', title: _('Join a Wi-Fi network'), page: 'hardware', section: 'wi-fi',
        steps: [
            {say: _('Open this menu, here at the top right.'), shell: 'quick-settings'},
            {say: _('Press the arrow beside Wi-Fi, then pick your network.'), shell: 'quick-wifi'},
        ],
    },
    {
        id: 'change-the-look', title: _('Change how the desktop looks'), page: 'themes', section: '',
        steps: [
            {say: _('Control has the desktop\'s settings. Open it, then find this page in its sidebar.'), app: 'amethystora-control', launcher: 'amethystora-control.desktop', find: 'Appearance'},
            {say: _('Pick a theme here. It changes at once, and you can always pick another.')},
        ],
    },
    {
        id: 'set-up-a-backup', title: _('Set up a backup'), page: 'security', section: 'backups',
        steps: [
            {say: _('Backups keeps your files. Open it, then press Status in its sidebar.'), app: 'amethystora-backups', launcher: 'amethystora-backups.desktop', find: 'Status'},
            {say: _('Press this. A terminal explains each step and asks before it changes anything.'), app: 'amethystora-backups', launcher: 'amethystora-backups.desktop', find: 'Set up a backup…'},
        ],
    },
    {
        id: 'update-now', title: _('Update this computer'), page: 'updates', section: 'updating-now',
        steps: [
            {say: _('System Updates updates everything at once. Open it, then press this.'), app: 'amethystora-update', launcher: 'amethystora-update.desktop', find: 'Update now'},
        ],
    },
    {
        id: 'something-wrong', title: _('Say that something is wrong'), page: 'troubleshooting', section: 'when-something-is-wrong',
        steps: [
            {say: _('Everything of the system\'s is in this menu, here at the top left.'), shell: 'logo-menu'},
            {say: _('Press Something\'s wrong, and pick what it is.')},
        ],
    },
];
