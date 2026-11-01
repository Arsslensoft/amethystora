// The three profiles as their keys say, and the words the Focus tile and the pill's card use for each. A profile is
// the eleven keys under its name in org.gnome.shell.extensions.amethystora-ui.focus (work-banners, work-apps, ...);
// "off" is every switch as the person set it.

import Shell from 'gi://Shell';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import {IDS as ACCENT_IDS} from '../theme/accents.js';

export const SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.focus';
export const PROFILES = ['work', 'games', 'sleep'];
export const CHOICES = ['off', ...PROFILES];
export const FIELDS = ['banners', 'apps', 'sounds', 'dock', 'lighter', 'night-light', 'power', 'accent', 'wallpaper',
    'grey', 'set'];
const LAUNCHER = /^[A-Za-z0-9._+-]+\.desktop$/;

// Every switch as the person set it
export const OFF = Object.freeze({
    id: 'off', banners: 'shown', apps: [], sounds: true, dock: 'as-set', lighter: false, night: 'as-set',
    power: 'as-set', accent: 'theme', wallpaper: 'as-set', grey: false, set: '',
});

// A profile's switches, read from its keys; a value a key cannot hold falls back to "as set"
export function readProfile(settings, id) {
    if (!PROFILES.includes(id))
        return OFF;
    const s = key => settings.get_string(`${id}-${key}`);
    const b = key => settings.get_boolean(`${id}-${key}`);
    const accent = s('accent');
    return {
        id,
        banners: s('banners'),
        apps: settings.get_strv(`${id}-apps`).filter(app => LAUNCHER.test(app)),
        sounds: b('sounds'),
        dock: s('dock'),
        lighter: b('lighter'),
        night: s('night-light'),
        power: s('power'),
        accent: ACCENT_IDS.includes(accent) ? accent : 'theme',
        wallpaper: s('wallpaper'),
        grey: b('grey'),
        set: s('set'),
    };
}

// A profile's name as the chips and the pill say it; called only from methods
export function profileName(id) {
    return {off: _('Off'), work: _('Work'), games: _('Games'), sleep: _('Sleep')}[id] ?? id;
}

// An accent as it follows another phrase on a line ("Power saver, an amber accent")
function accentPhrase(id) {
    return {
        amethyst: _('an amethyst accent'),
        iris: _('an iris accent'),
        sky: _('a sky accent'),
        lagoon: _('a lagoon accent'),
        jade: _('a jade accent'),
        moss: _('a moss accent'),
        amber: _('an amber accent'),
        ember: _('an ember accent'),
        coral: _('a coral accent'),
        rose: _('a rose accent'),
        orchid: _('an orchid accent'),
        slate: _('a slate accent'),
    }[id] ?? null;
}

// A phrase that starts a line
function capital(text) {
    return text ? text[0].toLocaleUpperCase() + text.slice(1) : text;
}

// The names of the apps a profile lets through that a person would know (a launcher that is hidden, such as the
// calendar's reminders, says nothing), then Clocks, which always comes through
function appNames(apps) {
    const system = Shell.AppSystem.get_default();
    const names = [];
    for (const id of apps) {
        if (/^org\.gnome\.clocks(\.desktop)?$/.test(id))
            continue;
        const app = system.lookup_app(id);
        const info = app?.get_app_info();
        if (app && info?.should_show())
            names.push(app.get_name());
    }
    names.push(_('Clocks'));
    return [...new Set(names)];
}

// What a profile does, as up to four short lines: the board's words with the shipped profiles
export function profileLines(profile) {
    const lines = [];
    if (profile.banners === 'apps')
        lines.push(_('Banners only from %s and calls').format(appNames(profile.apps).join(', ')));
    else if (profile.banners === 'held')
        lines.push(_('Banners held; calls and alarms come through'));
    if (!profile.sounds && profile.banners !== 'held')
        lines.push(_('Notification sounds off'));
    if (profile.banners === 'apps')
        lines.push(_('The rest waits in the list'));
    if (profile.dock === 'touch')
        lines.push(_('The dock hides while a window touches it'));

    // The screen, and the rest of the desktop
    const screen = [];
    if (profile.night === 'paused')
        screen.push(_('Night Light paused'));
    else if (profile.night === 'warmest')
        screen.push(_('Night Light at its warmest'));
    if (profile.wallpaper === 'night')
        screen.push(_('the night wallpaper'));
    if (profile.grey)
        screen.push(_('the screen in grey'));
    const rest = [];
    if (profile.power === 'performance')
        rest.push(_('performance power'));
    else if (profile.power === 'power-saver')
        rest.push(_('power saver'));
    const accent = profile.accent !== 'theme' ? accentPhrase(profile.accent) : null;
    if (accent)
        rest.push(accent);
    if (profile.lighter)
        rest.push(_('solid glass and shorter motion'));
    const join = parts => capital(parts.join(', '));
    if (screen.length === 1 && rest.length === 1) {
        lines.push(join([...rest, ...screen]));
    } else {
        if (screen.length)
            lines.push(join(screen));
        if (rest.length)
            lines.push(join(rest));
    }
    if (!lines.length)
        lines.push(_('Notifications, sounds and the desktop as you set them'));
    return lines;
}
