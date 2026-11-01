// Now playing: what a media player says it plays, from GNOME's own list of players (the one its calendar shows), in
// the pill with its cover, its title and a small level meter, and in the card with the buttons to go back, pause and
// go on. A player that is paused stays for ten minutes, then goes. The meter moves for four beats when a track starts
// and while the pointer is on the pill, and rests otherwise: a bar moving all afternoon would redraw the top bar all
// afternoon.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import Graphene from 'gi://Graphene';
import Pango from 'gi://Pango';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as Mpris from 'resource:///org/gnome/shell/ui/mpris.js';
import {getMixerControl} from 'resource:///org/gnome/shell/ui/status/volume.js';

import {buttonRow, cardButton, retire} from './clock.js';
import {plain} from './text.js';

// A paused player stays this long (seconds) after it last played
const PAUSED_FOR = 10 * 60;
// The meter: four bars, each rising from .35 to full and back in 900 ms, started this many ms apart, for four beats,
// then resting at these heights
const BAR_DELAYS = [0, 200, 450, 100];
const BAR_REST = [0.55, 1, 0.7, 0.4];
const BAR_LOW = 0.35;
const BEAT = 900;
const BEATS = 4;
// CSS's ease-in-out, which the board's meter moves with
const EASE_IN_OUT = [new Graphene.Point({x: 0.42, y: 0}), new Graphene.Point({x: 0.58, y: 1})];

// One list of players for the shell's life when GNOME's own is not where it is looked for, so that turning the
// module off and on does not make another
let fallbackSource = null;

function playerSource() {
    const own = Main.panel.statusArea.dateMenu?._messageList?._messageView?._mediaSource;
    if (own instanceof Mpris.MprisSource)
        return own;
    console.warn('Amethystora UI: the pill did not find GNOME\'s list of media players and keeps its own');
    fallbackSource ??= new Mpris.MprisSource();
    return fallbackSource;
}

// A player's properties, read without failing on one that has just gone
function read(player, name, fallback) {
    try {
        return player[name] ?? fallback;
    } catch {
        return fallback;
    }
}

// The cover's file, when the player gives a local one whose name is safe to put in a stylesheet: a picture from
// anywhere else is not fetched
export function coverPath(url) {
    if (typeof url !== 'string' || !url.startsWith('file://'))
        return null;
    let path;
    try {
        [path] = GLib.filename_from_uri(url);
    } catch {
        return null;
    }
    if (typeof path !== 'string' || !/^\/[A-Za-z0-9_./ +~@,%=-]*$/.test(path) || path.includes('/../'))
        return null;
    return path;
}

// A picture of the track: its cover with round corners, or the player's icon, or a note
function artwork(player, size, styleClass) {
    const path = coverPath(read(player, 'trackCoverUrl', ''));
    if (path) {
        return new St.Widget({
            style_class: styleClass,
            style: `background-image: url("${path}");`,
            y_align: Clutter.ActorAlign.CENTER,
        });
    }
    const gicon = read(player, 'app', null)?.get_icon?.() ?? null;
    const icon = new St.Icon({
        style_class: `${styleClass}-icon`,
        icon_size: size,
        y_align: Clutter.ActorAlign.CENTER,
        ...(gicon ? {gicon} : {icon_name: 'audio-x-generic-symbolic'}),
    });
    return icon;
}

// m:ss
function position(micro) {
    const s = Math.max(0, Math.floor(micro / 1e6));
    return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
}

export class Media {
    // phoneSource: the phone's players (modules/connect/players.js), or null
    constructor(activities, reduceMotion, phoneSource = () => null) {
        this._activities = activities;
        this._reduce = reduceMotion;
        this._phoneSource = phoneSource;
        this.id = 'media';
        this.row = {icon: 'audio-x-generic-symbolic', text: ''};
    }

    enable() {
        this._players = new Map();
        this._current = null;
        this._source = playerSource();
        this._phones = this._phoneSource();
        for (const source of [this._source, this._phones].filter(Boolean)) {
            source.connectObject(
                'player-added', (s, player) => this._add(player),
                'player-removed', (s, player) => this._remove(player),
                this);
            for (const player of source.players)
                this._add(player);
        }
    }

    disable() {
        this._source?.disconnectObject(this);
        this._source = null;
        this._phones?.disconnectObject(this);
        this._phones = null;
        for (const player of this._players?.keys() ?? [])
            player.disconnectObject(this);
        this._players = null;
        this._stopPausedTimer();
        this._end();
    }

    _add(player) {
        if (this._players.has(player))
            return;
        this._players.set(player, {played: 0, track: ''});
        player.connectObject('changed', () => this._changed(player), this);
        this._changed(player);
    }

    _remove(player) {
        player.disconnectObject(this);
        this._players.delete(player);
        this._choose();
    }

    _changed(player) {
        const info = this._players?.get(player);
        if (!info)
            return;
        const status = read(player, 'status', '');
        const track = `${read(player, 'trackTitle', '')}\u0000${read(player, 'trackArtists', []).join?.(',')}`;
        const newTrack = track !== info.track;
        info.track = track;
        if (status === 'Playing')
            info.played = GLib.get_monotonic_time();
        if (status === 'Playing' && (newTrack || info.status !== 'Playing'))
            info.started = true;
        info.status = status;
        this._choose();
    }

    // The player shown: the one playing that started most recently, else the one paused most recently, for ten
    // minutes after it last played
    _choose() {
        if (!this._players)
            return;
        const now = GLib.get_monotonic_time();
        let best = null;
        let bestInfo = null;
        for (const [player, info] of this._players) {
            const playing = info.status === 'Playing';
            const paused = info.status === 'Paused' && info.played > 0 && now - info.played < PAUSED_FOR * 1e6;
            if (!playing && !paused)
                continue;
            if (!best || (playing && bestInfo.status !== 'Playing') ||
                (playing === (bestInfo.status === 'Playing') && info.played > bestInfo.played)) {
                best = player;
                bestInfo = info;
            }
        }
        if (!best) {
            this._stopPausedTimer();
            this._end();
            return;
        }
        if (bestInfo.status === 'Paused')
            this._startPausedTimer(bestInfo.played);
        else
            this._stopPausedTimer();
        const switched = best !== this._current;
        this._current = best;
        this._update(switched || bestInfo.started);
        bestInfo.started = false;
    }

    _startPausedTimer(played) {
        this._stopPausedTimer();
        const left = PAUSED_FOR - (GLib.get_monotonic_time() - played) / 1e6;
        this._pausedTimer = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, Math.max(1, Math.ceil(left)), () => {
            this._pausedTimer = 0;
            this._choose();
            return GLib.SOURCE_REMOVE;
        });
    }

    _stopPausedTimer() {
        if (this._pausedTimer)
            GLib.source_remove(this._pausedTimer);
        this._pausedTimer = 0;
    }

    get playing() {
        return this._current ? read(this._current, 'status', '') === 'Playing' : false;
    }

    get title() {
        return plain(read(this._current, 'trackTitle', ''), 80);
    }

    get artist() {
        const artists = read(this._current, 'trackArtists', []);
        return plain(Array.isArray(artists) ? artists.join(', ') : '', 80);
    }

    get text() {
        return this.artist ? `${this.title} · ${this.artist}` : this.title;
    }

    get accessible() {
        return this.artist
            ? _('Now playing: %s by %s').format(this.title, this.artist)
            : _('Now playing: %s').format(this.title);
    }

    // The pill's content, made once and brought up to date
    pill() {
        if (!this._content)
            this._build();
        return this._content;
    }

    _build() {
        this._content = new St.BoxLayout({style_class: 'amethystora-pill-content amethystora-pill-media'});
        this._art = new St.Bin({style_class: 'amethystora-pill-art-bin', y_align: Clutter.ActorAlign.CENTER});
        this._label = new St.Label({style_class: 'amethystora-pill-media-title', y_align: Clutter.ActorAlign.CENTER});
        this._label.clutter_text.ellipsize = Pango.EllipsizeMode.END;
        this._bars = new St.BoxLayout({style_class: 'amethystora-pill-bars', y_align: Clutter.ActorAlign.CENTER});
        for (let i = 0; i < 4; i++) {
            this._bars.add_child(new St.Widget({
                style_class: 'amethystora-pill-bar',
                y_align: Clutter.ActorAlign.END,
                pivot_point: new Graphene.Point({x: 0.5, y: 1}),
            }));
        }
        this._state = new St.Icon({style_class: 'amethystora-pill-state', icon_size: 12, y_align: Clutter.ActorAlign.CENTER});
        this._state._pillAction = () => this._current?.playPause();
        this._content.add_child(this._art);
        this._content.add_child(this._label);
        this._content.add_child(this._bars);
        this._content.add_child(this._state);
    }

    _update(moveBars) {
        const player = this._current;
        if (!this._content)
            this._build();
        const cover = read(player, 'trackCoverUrl', '');
        if (cover !== this._cover || !this._art.child) {
            this._cover = cover;
            this._art.set_child(artwork(player, 18, 'amethystora-pill-art'));
        }
        this._label.text = this.text;
        const playing = this.playing;
        this._state.icon_name = playing ? 'media-playback-pause-symbolic' : 'media-playback-start-symbolic';
        this.row = {icon: read(player, 'app', null)?.get_icon?.() ?? 'audio-x-generic-symbolic', text: _('Now playing · %s').format(this.title)};
        this.paused = !playing;
        const first = this._activities.get('media') !== this;
        this._activities.set('media', this);
        if (!first)
            this._activities.update('media');
        // The meter moves once its content is in the pill: an actor not shown would end its beats at once
        if (moveBars && playing)
            this.moveBars();
        else if (!playing)
            this._restBars(BAR_LOW);
        else if (!this._barsMoving())
            this._restBars();
        this._card?.update();
    }

    _end() {
        this._current = null;
        this._card = null;
        if (this._activities.get('media') === this)
            this._activities.set('media', null);
        retire(this._content);
        // Its parts go with it: nothing may touch them once the pill has let them go
        this._content = null;
        this._art = this._label = this._bars = this._state = null;
        this._cover = null;
    }

    hovered(hover) {
        if (hover && this.playing)
            this.moveBars();
    }

    _barsMoving() {
        return this._bars?.get_children().some(bar => bar.get_transition('scale-y')) ?? false;
    }

    get barsMoving() {
        return this._barsMoving();
    }

    // Four beats of the meter, then rest
    moveBars() {
        if (!this._bars || this._barsMoving())
            return;
        if (this._reduce()) {
            this._restBars();
            return;
        }
        this._bars.get_children().forEach((bar, i) => {
            bar.remove_transition('scale-y');
            bar.scale_y = BAR_LOW;
            bar.ease({
                scale_y: 1,
                delay: BAR_DELAYS[i],
                duration: BEAT / 2,
                repeatCount: 2 * BEATS - 1,
                autoReverse: true,
                mode: Clutter.AnimationMode.EASE_IN_OUT,
                onComplete: () => bar.ease({
                    scale_y: BAR_REST[i],
                    duration: BEAT / 2,
                    mode: Clutter.AnimationMode.EASE_IN_OUT,
                }),
            });
            bar.get_transition('scale-y')?.set_cubic_bezier_progress(...EASE_IN_OUT);
        });
    }

    _restBars(height = null) {
        this._bars?.get_children().forEach((bar, i) => {
            bar.remove_transition('scale-y');
            bar.scale_y = height ?? BAR_REST[i];
        });
    }

    card(api) {
        const card = new MediaCard(this, api);
        this._card = card;
        card.connect('destroy', () => {
            if (this._card === card)
                this._card = null;
        });
        return card;
    }

    get player() {
        return this._current;
    }
}

// The card of what plays: cover, title, artist and app, where it is in the track, and the three buttons
const MediaCard = GObject.registerClass(
class AmethystoraPillMediaCard extends St.BoxLayout {
    _init(media, api) {
        super._init({style_class: 'amethystora-pill-card-body', orientation: Clutter.Orientation.VERTICAL});
        this._media = media;
        this._api = api;
        this._position = 0;

        const head = new St.BoxLayout({style_class: 'amethystora-pill-media-head'});
        this._art = new St.Bin({style_class: 'amethystora-pill-cover-bin'});
        const words = new St.BoxLayout({
            style_class: 'amethystora-pill-media-words',
            orientation: Clutter.Orientation.VERTICAL,
            y_align: Clutter.ActorAlign.CENTER,
            x_expand: true,
        });
        this._title = new St.Label({style_class: 'amethystora-pill-card-title'});
        this._title.clutter_text.ellipsize = Pango.EllipsizeMode.END;
        this._sub = new St.Label({style_class: 'amethystora-pill-card-text'});
        this._sub.clutter_text.ellipsize = Pango.EllipsizeMode.END;
        words.add_child(this._title);
        words.add_child(this._sub);
        head.add_child(this._art);
        head.add_child(words);
        this.add_child(head);

        this._progress = new St.BoxLayout({
            style_class: 'amethystora-pill-progress-box',
            orientation: Clutter.Orientation.VERTICAL,
        });
        this._track = new St.Widget({style_class: 'amethystora-pill-progress', x_expand: true});
        this._fill = new St.Widget({style_class: 'amethystora-pill-progress-fill'});
        this._track.add_child(this._fill);
        this._track.connect('notify::width', () => this._drawProgress());
        const times = new St.BoxLayout({style_class: 'amethystora-pill-progress-times'});
        this._at = new St.Label({style_class: 'amethystora-pill-card-small', x_expand: true});
        this._lengthLabel = new St.Label({style_class: 'amethystora-pill-card-small'});
        times.add_child(this._at);
        times.add_child(this._lengthLabel);
        this._progress.add_child(this._track);
        this._progress.add_child(times);
        this.add_child(this._progress);

        this._back = cardButton(_('Back'), () => media.player?.previous());
        this._play = cardButton(_('Pause'), () => media.player?.playPause(), {primary: true});
        this._next = cardButton(_('Next'), () => media.player?.next());
        this.add_child(buttonRow([this._back, this._play, this._next], {center: true}));

        this._output = new St.Label({style_class: 'amethystora-pill-card-text amethystora-pill-card-center', x_align: Clutter.ActorAlign.CENTER});
        this.add_child(this._output);

        this._watchPosition();
        this.update();
        this.connect('destroy', () => this._stop());
    }

    // Asked once, then followed by the player's own Seeked signal; between them the bar moves on by itself
    _watchPosition() {
        const player = this._media.player;
        const bus = player?._busName;
        if (typeof bus !== 'string') {
            // A phone's player says where it was each time it says anything
            const micro = read(player, 'trackPositionMicro', null);
            if (Number.isFinite(micro))
                this._setPosition(micro);
            return;
        }
        this._bus = bus;
        this._cancellable = new Gio.Cancellable();
        Gio.DBus.session.call(bus, '/org/mpris/MediaPlayer2', 'org.freedesktop.DBus.Properties', 'Get',
            new GLib.Variant('(ss)', ['org.mpris.MediaPlayer2.Player', 'Position']), new GLib.VariantType('(v)'),
            Gio.DBusCallFlags.NO_AUTO_START, 2000, this._cancellable, (connection, result) => {
                try {
                    const [value] = connection.call_finish(result).deepUnpack();
                    const micro = Number(value.deepUnpack());
                    if (Number.isFinite(micro))
                        this._setPosition(micro);
                } catch {}
            });
        this._seeked = Gio.DBus.session.signal_subscribe(bus, 'org.mpris.MediaPlayer2.Player', 'Seeked',
            '/org/mpris/MediaPlayer2', null, Gio.DBusSignalFlags.NONE, (c, s, p, i, n, params) => {
                const micro = Number(params.deepUnpack()[0]);
                if (Number.isFinite(micro))
                    this._setPosition(micro);
            });
    }

    _setPosition(micro) {
        this._position = micro;
        this._since = GLib.get_monotonic_time();
        this._drawProgress();
    }

    _stop() {
        this._cancellable?.cancel();
        this._cancellable = null;
        if (this._seeked)
            Gio.DBus.session.signal_unsubscribe(this._seeked);
        this._seeked = 0;
        if (this._tick)
            GLib.source_remove(this._tick);
        this._tick = 0;
    }

    update() {
        const media = this._media;
        const player = media.player;
        if (!player)
            return;
        const cover = read(player, 'trackCoverUrl', '');
        if (cover !== this._cover || !this._art.child) {
            this._cover = cover;
            this._art.set_child(artwork(player, 72, 'amethystora-pill-cover'));
        }
        this._title.text = media.title;
        const app = plain(read(player, 'source', '') || (read(player, 'app', null)?.get_name?.() ?? player._mprisProxy?.Identity ?? ''), 60);
        this._sub.text = [media.artist, app].filter(Boolean).join(' · ');

        let length = Number(read(player, 'trackLengthMicro', 0));
        try {
            const metadata = player._playerProxy?.Metadata;
            length ||= Number(metadata?.['mpris:length']?.deepUnpack?.() ?? 0);
        } catch {}
        this._lengthMicro = Number.isFinite(length) && length > 0 ? length : 0;
        this._progress.visible = this._lengthMicro > 0;
        this._lengthLabel.text = this._lengthMicro > 0 ? position(this._lengthMicro) : '';

        const playing = media.playing;
        this._play.label = playing ? _('Pause') : _('Play');
        this._play.accessible_name = this._play.label;
        this._sensitive(this._back, read(player, 'canGoPrevious', false));
        this._sensitive(this._next, read(player, 'canGoNext', false));

        const sink = getMixerControl()?.get_default_sink?.();
        const output = plain(sink?.get_description?.() ?? '', 60);
        this._output.text = output ? _('Playing on %s').format(output) : '';
        this._output.visible = output !== '';

        // A phone's player says where it is with each change, and is taken at its word
        const phoneAt = typeof player._busName === 'string' ? null : read(player, 'trackPositionMicro', null);
        if (Number.isFinite(phoneAt) && phoneAt !== this._phoneAt) {
            this._phoneAt = phoneAt;
            this._wasPlaying = playing;
            this._setPosition(phoneAt);
        }
        // While it plays, the bar moves on once a second, from where the player last said it was
        if (this._wasPlaying !== playing) {
            this._position = this._currentPosition();
            this._since = GLib.get_monotonic_time();
            this._wasPlaying = playing;
        }
        if (playing && this._lengthMicro > 0 && !this._tick) {
            this._tick = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, 1, () => {
                this._drawProgress();
                return GLib.SOURCE_CONTINUE;
            });
        } else if ((!playing || this._lengthMicro === 0) && this._tick) {
            GLib.source_remove(this._tick);
            this._tick = 0;
        }
        this._drawProgress();
    }

    _sensitive(button, sensitive) {
        button.reactive = !!sensitive;
        button.can_focus = !!sensitive;
        if (sensitive)
            button.remove_style_pseudo_class('insensitive');
        else
            button.add_style_pseudo_class('insensitive');
    }

    _currentPosition() {
        const elapsed = this._wasPlaying && this._since ? GLib.get_monotonic_time() - this._since : 0;
        return this._position + elapsed;
    }

    _drawProgress() {
        if (!this._lengthMicro)
            return;
        const at = Math.min(this._currentPosition(), this._lengthMicro);
        this._at.text = position(at);
        if (!this._track.get_stage())
            return;
        const width = this._track.width;
        if (width > 0)
            this._fill.set_size(Math.max(this._track.height, Math.round(width * at / this._lengthMicro)), this._track.height);
    }
});
