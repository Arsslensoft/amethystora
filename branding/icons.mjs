// The Amethystora icon packs: one set of glyphs (icons/glyphs.mjs), each icon in colours of its own, drawn eight ways.
//
//   Glass    the glyph filled with its gradient, its details lighter, a glass sheen inside and a shadow below
//   Neon     the glyph's outline and its details lit in a three-colour prism, glowing
//   Duotone  the glyph in its gradient and its details in a contrasting one
//   Line     one even outline, in colours that read on every theme's ground
//   Clay     soft matte volumes: a highlight above, a shade below, a soft shadow
//   Muted    Glass in quieter colours with half the sheen, for the themes of low chroma
//   Glass for light, Neon for light   the same drawings with every colour deepened, to read on a light ground
//
// Each theme names the pack that suits its colours and style (themes/*/icons.theme); every pack's colours are held to
// 3:1 against the grounds it is meant for, which the board of 2026-10-25 measured (22 ramps, 10 grounds).
//
// They cover Amethystora's apps, GNOME's, the system's and other makers' apps, each drawn by what it does, folders,
// file types, devices, dialogs and categories. An app whose icon is its maker's mark (LOGOS) keeps the one it ships,
// which the packs reach through Inherits (hicolor), and the symbolic icons of the top bar and the menus are Adwaita's.
//
//   node branding/icons.mjs            writes the packs into system_files/shared/usr/share/icons
//   node branding/icons.mjs --preview  also writes a sheet of every icon in every pack, and prints where
import {mkdirSync, rmSync, writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {dirname, join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {DRAW} from './icons/glyphs.mjs';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');
const ICONS = join(ROOT, 'system_files/shared/usr/share/icons');

// The colours: each ramp is the gradient the glyph is filled with, the prism Neon lights it in, and the contrasting
// gradient Duotone draws its details in
const RAMPS = {
    sunset: [['#ffb347', '#ff5f6d'], ['#ffe259', '#ffa751', '#ff5f6d'], ['#5be7ff', '#4facfe']],
    ocean: [['#36d1dc', '#5b86e5'], ['#a8ff78', '#36d1dc', '#5b86e5'], ['#ffd86f', '#fc6262']],
    mint: [['#38ef7d', '#11998e'], ['#f9f871', '#38ef7d', '#11998e'], ['#f6d365', '#fda085']],
    orchid: [['#ff6ec4', '#7873f5'], ['#ffd3a5', '#ff6ec4', '#7873f5'], ['#d4fc79', '#96e6a1']],
    amber: [['#ffd200', '#f7971e'], ['#fff6b7', '#ffd200', '#f7971e'], ['#6a11cb', '#2575fc']],
    dusk: [['#b06ab3', '#4568dc'], ['#ff9a9e', '#b06ab3', '#4568dc'], ['#fddb92', '#f6d365']],
    emerald: [['#00f260', '#0575e6'], ['#e0ff4f', '#00f260', '#0575e6'], ['#ffe29f', '#ffa99f']],
    flame: [['#fc466b', '#3f5efb'], ['#ffb199', '#fc466b', '#3f5efb'], ['#c2e9fb', '#a1c4fd']],
    sky: [['#00f2fe', '#4facfe'], ['#c2ffd8', '#00f2fe', '#4facfe'], ['#ffecd2', '#fcb69f']],
    blossom: [['#f093fb', '#f5576c'], ['#ffe0f7', '#f093fb', '#f5576c'], ['#fffde4', '#fbd786']],
    grape: [['#da22ff', '#9733ee'], ['#ffb3ff', '#da22ff', '#9733ee'], ['#f9f871', '#7df9c4']],
    lagoon: [['#00c6ff', '#0072ff'], ['#8fffef', '#00c6ff', '#0072ff'], ['#ffe985', '#fa742b']],
    coral: [['#ff9966', '#ff5e62'], ['#ffd194', '#ff9966', '#ff5e62'], ['#a1ffce', '#5ee7df']],
    lime: [['#a8e063', '#2fb36d'], ['#f9f871', '#a8e063', '#2fb36d'], ['#ffc3f8', '#f093fb']],
    cherry: [['#ff512f', '#dd2476'], ['#ffc371', '#ff512f', '#dd2476'], ['#fdfcfb', '#b8f1ff']],
    royal: [['#4776e6', '#8e54e9'], ['#7ee8fa', '#4776e6', '#8e54e9'], ['#ffd3a5', '#fd6585']],
    aqua: [['#43e97b', '#38f9d7'], ['#f9f871', '#43e97b', '#38f9d7'], ['#5f72ff', '#9a5cff']],
    peach: [['#f6d365', '#fda085'], ['#fff1b8', '#f6d365', '#fda085'], ['#7f7fd5', '#86a8e7']],
    magenta: [['#ec008c', '#fc6767'], ['#ffb3d9', '#ec008c', '#fc6767'], ['#fff6b7', '#f6d365']],
    indigo: [['#667eea', '#764ba2'], ['#a1c4fd', '#667eea', '#8e6bd8'], ['#f9f871', '#ffc371']],
    ice: [['#89f7fe', '#66a6ff'], ['#e0fcff', '#89f7fe', '#66a6ff'], ['#ff9a9e', '#fecfef']],
    gold: [['#f7b733', '#fc4a1a'], ['#fff3b0', '#f7b733', '#fc4a1a'], ['#89f7fe', '#66a6ff']],
    // The Vault's door (its icon board, option A): brass, with the dial in a paler brass and, for Duotone, in a violet
    brass: [['#f5c76a', '#d98a2b'], ['#fff3cf', '#f5c76a', '#d98a2b'], ['#a98bf2', '#6b6fe0']],
    // The folder colours of Files' scripts menu (board 35): the board's six swatches, light over dark, the prism from a
    // near-white of the hue (Sky's a little bluer than the board's, which Neon would leave too near Slate), and a
    // contrasting pair for Duotone's details
    'folder-amethyst': [['#c9a2ff', '#9148f0'], ['#e4d0ff', '#c9a2ff', '#9148f0'], ['#ffd27a', '#e08a2c']],
    'folder-sky': [['#a9cdff', '#3d68e6'], ['#cfe2ff', '#a9cdff', '#3d68e6'], ['#ffd27a', '#e08a2c']],
    'folder-jade': [['#9be6c3', '#1f9e6e'], ['#d4f7e6', '#9be6c3', '#1f9e6e'], ['#ffb0c0', '#d63b5c']],
    'folder-amber': [['#ffd27a', '#e08a2c'], ['#fff0c9', '#ffd27a', '#e08a2c'], ['#a9cdff', '#3d68e6']],
    'folder-coral': [['#ffb0c0', '#d63b5c'], ['#ffe0e6', '#ffb0c0', '#d63b5c'], ['#9be6c3', '#1f9e6e']],
    'folder-slate': [['#cdd3e0', '#5b6680'], ['#eef1f6', '#cdd3e0', '#5b6680'], ['#c9a2ff', '#9148f0']],
};
// The folder colours, which have to stay told apart in every pack
const FOLDER_COLOURS = ['amethyst', 'sky', 'jade', 'amber', 'coral', 'slate'];
const FOLDER_RAMPS = new Set(FOLDER_COLOURS.map(colour => RAMPS[`folder-${colour}`]));

// Every icon: its name, the drawing, the ramp, and the other names it answers to. A file type is named as
// shared-mime-info's generic icons are, so it covers every type that falls back on that name.
const SET = {
    apps: [
        ['amethystora-docs', 'manual', 'dusk'],
        ['amethystora-security-status', 'security', 'emerald'],
        ['amethystora-control', 'control', 'flame'],
        ['amethystora-update', 'update', 'lagoon', ['system-software-update', 'system-software-install', 'software-update-available']],
        ['amethystora-logs', 'logs', 'aqua', ['utilities-log-viewer', 'org.gnome.Logs']],
        ['amethystora-notes', 'notes', 'gold', ['accessories-notes']],
        ['amethystora-backups', 'backups', 'royal'],
        ['amethystora-vault', 'door', 'brass'],
        ['amethystora-phone', 'phone-link', 'grape'],
        ['amethystora-community', 'community', 'magenta'],
        ['org.gnome.Nautilus', 'files', 'sunset', ['system-file-manager']],
        ['org.gnome.Settings', 'settings', 'ocean', ['preferences-system', 'preferences-desktop', 'gnome-control-center']],
        ['org.gnome.Ptyxis', 'terminal', 'mint', ['utilities-terminal', 'org.gnome.Terminal', 'org.gnome.Console']],
        ['org.gnome.SystemMonitor', 'monitor', 'cherry', ['utilities-system-monitor', 'gnome-system-monitor', 'org.gnome.Sysprof']],
        ['org.gnome.DiskUtility', 'drive', 'indigo', ['disk-utility', 'gnome-disks']],
        ['org.gnome.Tour', 'compass', 'peach'],
        ['org.gnome.tweaks', 'wrench', 'lime'],
        ['org.freedesktop.MalcontentControl', 'guardian', 'grape'],
        ['firewall-config', 'wall', 'coral'],
        ['input-remapper', 'keyboard', 'dusk'],
        ['gnome-color-manager', 'palette', 'blossom', ['preferences-color']],
        ['rygel', 'broadcast', 'lagoon', ['rygel-full']],
        ['help-browser', 'lifebuoy', 'cherry', ['org.gnome.Yelp', 'help-contents', 'yelp', 'system-help']],
        ['org.gnome.Calculator', 'calculator', 'amber', ['accessories-calculator', 'gnome-calculator']],
        ['org.gnome.Calendar', 'calendar', 'cherry', ['office-calendar']],
        ['org.gnome.Characters', 'smile', 'peach', ['accessories-character-map']],
        ['org.gnome.Connections', 'remote', 'royal', ['preferences-desktop-remote-desktop']],
        ['org.gnome.Contacts', 'contact', 'coral', ['office-address-book']],
        ['org.gnome.FileRoller', 'archive', 'gold', ['file-roller']],
        ['org.gnome.Firmware', 'chip', 'lime', ['org.freedesktop.fwupd', 'io.github.thetumultuousunicornofdarkness.cpu-x']],
        ['org.gnome.Loupe', 'photo', 'magenta', ['image-viewer', 'multimedia-photo-viewer']],
        ['org.gnome.Maps', 'map', 'aqua'],
        ['org.gnome.Papers', 'pages', 'flame', ['document-viewer', 'org.gnome.Evince', 'document-print-preview', 'org.gnome.NautilusPreviewer']],
        ['org.gnome.Showtime', 'play', 'grape', ['multimedia-video-player', 'video-player', 'org.gnome.Totem']],
        ['org.gnome.SimpleScan', 'scanner', 'ice'],
        ['org.gnome.Snapshot', 'camera', 'indigo', ['accessories-camera']],
        ['org.gnome.Decibels', 'notes2', 'blossom', ['multimedia-audio-player', 'audio-player']],
        ['org.gnome.TextEditor', 'editor', 'orchid', ['accessories-text-editor', 'text-editor', 'org.gnome.gedit']],
        ['org.gnome.Weather', 'weather', 'sky'],
        ['org.gnome.baobab', 'pie', 'gold', ['disk-usage-analyzer', 'baobab']],
        ['org.gnome.clocks', 'alarm', 'lagoon', ['preferences-system-time']],
        ['org.gnome.font-viewer', 'letter', 'dusk', ['preferences-desktop-font']],
        ['org.gnome.Builder', 'code', 'royal'],
        ['application-default-icon', 'app', 'royal'],

        // Other makers' apps, each drawn by what it does
        ['org.kde.dolphin', 'files', 'sunset', ['thunar', 'org.xfce.thunar', 'nemo', 'caja', 'pcmanfm', 'pcmanfm-qt']],
        ['org.kde.konsole', 'terminal', 'mint', ['konsole', 'xfce4-terminal', 'org.xfce.terminal', 'com.gexperts.Tilix', 'tilix',
            'com.raggesilver.BlackBox', 'app.devsuite.Prompt', 'gnome-terminal', 'mate-terminal', 'lxterminal', 'qterminal']],
        ['org.kde.kwrite', 'editor', 'orchid', ['kwrite', 'org.xfce.mousepad', 'mousepad', 'pluma', 'xed', 'org.x.editor', 'gedit',
            'org.gnome.gitlab.somas.Apostrophe', 'com.github.fabiocolacio.marker']],
        ['org.kde.kcalc', 'calculator', 'amber', ['kcalc', 'galculator', 'qalculate', 'qalculate-gtk', 'io.github.Qalculate', 'mate-calc']],
        ['io.missioncenter.MissionCenter', 'monitor', 'cherry', ['net.nokyan.Resources', 'org.kde.plasma-systemmonitor', 'ksysguard', 'htop',
            'btop', 'nvtop', 'org.xfce.taskmanager', 'xfce4-taskmanager']],
        ['org.kde.systemsettings', 'settings', 'ocean', ['systemsettings', 'org.xfce.settings.manager', 'xfce4-settings-manager',
            'ca.desrt.dconf-editor', 'dconf-editor']],
        ['org.gnome.Software', 'store', 'peach', ['gnome-software', 'io.github.kolunmi.Bazaar', 'org.kde.discover', 'plasmadiscover']],
        ['io.github.flattool.Warehouse', 'package', 'gold'],
        ['com.github.tchx84.Flatseal', 'toggles', 'emerald'],
        ['gparted', 'drive', 'indigo', ['org.gnome.GParted', 'org.kde.partitionmanager', 'partitionmanager']],
        ['org.gnome.DejaDup', 'drive', 'emerald', ['deja-dup', 'org.gnome.World.PikaBackup', 'timeshift', 'com.borgbase.Vorta']],
        ['io.gitlab.adhami3310.Impression', 'usb', 'gold'],
        ['io.gitlab.metadatacleaner.metadatacleaner', 'broom', 'lime', ['org.bleachbit.BleachBit', 'bleachbit', 'org.kde.sweeper']],
        ['de.leopoldluley.Clapgrep', 'search', 'sky', ['catfish', 'org.xfce.catfish', 'org.kde.kfind', 'io.github.cboxdoerfer.FSearch', 'system-search']],
        ['io.github.getnf.embellish', 'letter', 'dusk', ['org.gnome.FontManager', 'font-manager']],
        ['me.iepure.devtoolbox', 'toolbox', 'gold', ['com.vysp3r.ProtonPlus']],
        ['com.mattjakeman.ExtensionManager', 'extension-store', 'orchid'],
        ['org.gnome.Extensions', 'extensions', 'lime', ['org.gnome.Shell.Extensions']],
        ['org.gnome.BrowserConnector', 'addon', 'sky'],
        ['it.mijorus.smile', 'smile', 'peach', ['com.tomjwatson.Emote']],
        ['page.tesk.Refine', 'wrench', 'lime'],
        ['com.ranfdev.DistroShelf', 'containers', 'ocean', ['io.github.dvlv.boxbuddyrs', 'distrobox', 'terminal-distrobox-icon']],
        ['org.gnome.Boxes', 'vm', 'indigo', ['gnome-boxes', 'virt-manager', 'org.virt_manager.virt-manager']],
        ['re.sonny.Workbench', 'code', 'royal', ['org.gnome.Meld', 'meld', 'org.gnome.GHex', 'ghex']],
        ['org.gnome.gitg', 'branch', 'coral', ['gitg', 'org.kde.kommit']],
        ['org.gnome.Geary', 'mail', 'lagoon', ['geary', 'org.gnome.Evolution', 'evolution', 'org.kde.kmail2', 'kmail', 'internet-mail']],
        ['org.gnome.Fractal', 'chat', 'aqua', ['org.gnome.Polari', 'polari', 'dev.geopjr.Tuba', 'org.kde.tokodon', 'org.kde.neochat',
            'internet-chat', 'internet-group-chat', 'empathy', 'com.rtosta.zapzap', 'com.github.IsmaelMartinez.teams_for_linux']],
        ['io.gitlab.news_flash.NewsFlash', 'feed', 'amber', ['com.gitlab.newsflash', 'org.gabmus.gfeeds', 'org.kde.akregator', 'akregator',
            'liferea', 'net.sourceforge.liferea', 'internet-news-reader']],
        ['de.haeckerfelix.Fragments', 'download', 'emerald', ['com.transmissionbt.Transmission', 'transmission', 'transmission-gtk',
            'transmission-qt', 'org.kde.ktorrent', 'ktorrent', 'io.github.giantpinkrobots.varia', 'org.nickvision.tubeconverter']],
        ['org.gnome.seahorse.Application', 'key', 'gold', ['seahorse', 'org.gnome.World.Secrets', 'com.belmoussaoui.Authenticator',
            'kwalletmanager', 'org.kde.kwalletmanager5', 'gcr-gnupg', 'gcr-key', 'gcr-key-pair', 'gcr-password']],
        ['gcr-smart-card', 'smartcard', 'ocean'],
        ['com.github.johnfactotum.Foliate', 'book', 'peach', ['info.febvre.Komikku', 'com.calibre_ebook.calibre', 'calibre-gui',
            'com.github.babluboy.bookworm', 'org.kde.arianna', 'accessories-dictionary', 'org.gnome.Dictionary']],
        ['com.github.PintaProject.Pinta', 'brush', 'blossom', ['com.github.maoschanz.drawing', 'org.kde.kolourpaint', 'kolourpaint', 'com.github.flxzt.rnote']],
        ['be.alexandervanhee.gradia', 'screenshot', 'flame', ['org.gnome.Screenshot', 'gnome-screenshot', 'org.kde.spectacle', 'spectacle',
            'org.ksnip.ksnip', 'applets-screenshooter']],
        ['io.github.seadve.Kooha', 'record', 'cherry'],
        ['org.gnome.SoundRecorder', 'microphone', 'cherry', ['gnome-sound-recorder']],
        ['com.github.finefindus.eyedropper', 'eyedropper', 'magenta', ['nl.hjdskes.gcolor3', 'gcolor3', 'org.kde.kcolorchooser']],
        ['io.github.nate_xyz.Paleta', 'palette', 'blossom'],
        ['app.drey.Dialect', 'translate', 'sky'],
        ['org.gnome.Music', 'notes2', 'blossom', ['gnome-music', 'org.gnome.Rhythmbox3', 'rhythmbox', 'org.gnome.Lollypop', 'lollypop',
            'io.bassi.Amberol', 'org.kde.elisa', 'elisa', 'com.github.neithern.g4music']],
        ['de.haeckerfelix.Shortwave', 'radio', 'coral'],
        ['org.gnome.Podcasts', 'broadcast', 'lagoon', ['org.kde.kasts', 'kasts']],
        ['io.github.celluloid_player.Celluloid', 'play', 'grape', ['celluloid', 'org.kde.haruna', 'haruna', 'com.github.rafostar.Clapper',
            'org.kde.dragonplayer', 'org.xfce.Parole', 'parole']],
        ['org.pitivi.Pitivi', 'film', 'flame', ['pitivi', 'org.gnome.gitlab.YaLTeR.VideoTrimmer', 'io.github.jliljebl.Flowblade']],
        ['org.gnome.eog', 'photo', 'magenta', ['eog', 'org.gnome.gThumb', 'gthumb', 'org.kde.gwenview', 'gwenview', 'org.xfce.ristretto',
            'ristretto', 'org.gnome.Shotwell', 'shotwell', 'org.gnome.Photos', 'gnome-photos', 'com.github.huluti.Curtail',
            'io.gitlab.theevilskeleton.Upscaler', 'io.gitlab.adhami3310.Converter']],
        ['org.kde.okular', 'pages', 'flame', ['okular', 'evince', 'atril', 'org.x.reader', 'xreader']],
        ['org.gnome.Cheese', 'camera', 'indigo', ['cheese', 'org.kde.kamoso']],
        ['org.kde.korganizer', 'calendar', 'cherry', ['korganizer']],
        ['org.kde.kaddressbook', 'contact', 'coral', ['kaddressbook']],
        ['org.gnome.Notes', 'notepad', 'amber', ['bijiben', 'org.gnome.Gnote', 'gnote', 'org.gnome.World.Iotas', 'com.toolstack.Folio']],
        ['io.github.alainm23.planify', 'tasks', 'emerald', ['io.github.mrvladus.List', 'org.gnome.Todo', 'org.gnome.Endeavour']],
        ['org.gnome.Solanum', 'alarm', 'lagoon', ['io.gitlab.idevecore.Pomodoro', 'org.gnome.Evolution-alarm-notify']],
        ['org.kde.ark', 'archive', 'gold', ['ark', 'xarchiver', 'engrampa', 'io.github.peazip.PeaZip']],
        ['org.gnome.Mines', 'gamepad', 'flame', ['gnome-mines', 'org.gnome.Sudoku', 'gnome-sudoku', 'org.gnome.Quadrapassel', 'org.gnome.Tali',
            'org.gnome.SwellFoop', 'org.gnome.TwentyFortyEight', 'org.gnome.Nibbles', 'org.gnome.Robots', 'org.gnome.Klotski',
            'org.gnome.Reversi', 'org.gnome.Taquin', 'org.gnome.Tetravex', 'org.gnome.LightsOff', 'org.gnome.Atomix', 'org.gnome.Hitori',
            'org.gnome.five-or-more', 'org.gnome.Four-in-a-row']],
        ['org.gnome.Aisleriot', 'cards', 'cherry', ['gnome-aisleriot', 'sol', 'org.gnome.Mahjongg', 'gnome-mahjongg', 'org.kde.kpat', 'kpat']],
        ['org.gnome.Chess', 'chess', 'dusk', ['gnome-chess', 'org.kde.knights']],
        ['orca', 'access', 'ocean', ['org.gnome.Orca', 'preferences-desktop-accessibility']],
        ['ibus', 'keyboard', 'indigo', ['ibus-setup', 'org.freedesktop.IBus.Setup', 'org.gnome.Tecla', 'preferences-desktop-keyboard',
            'preferences-desktop-keyboard-shortcuts', 'ibus-anthy', 'ibus-engine', 'ibus-hangul', 'ibus-keyboard', 'ibus-m17n', 'ibus-setup-hangul',
            'ibus-unikey', 'org.freedesktop.IBus.Chewing.Setup', 'production']],
        ['org.freedesktop.Piper', 'mouse', 'sky', ['piper', 'preferences-desktop-peripherals']],
        ['org.pulseaudio.pavucontrol', 'speaker', 'sunset', ['pavucontrol', 'multimedia-volume-control', 'com.saivert.pwvucontrol', 'com.rafaelmardojai.Blanket']],
        ['com.github.wwmm.easyeffects', 'equalizer', 'grape', ['easyeffects']],
        ['nm-connection-editor', 'wifi', 'lagoon', ['preferences-system-network', 'nm-device-wireless']],
        ['bluetooth', 'transfer', 'ocean', ['preferences-system-sharing', 'org.freedesktop.Passim']],
        ['network-vpn', 'vpn', 'royal', ['nm-vpn-standalone-lock']],
        ['preferences-system-privacy', 'lock', 'emerald'],
        ['preferences-system-notifications', 'bell', 'amber'],
        ['preferences-system-windows', 'stacked', 'indigo'],
        ['system-config-printer', 'printer', 'coral'],
        ['org.gnome.PowerStats', 'battery', 'lime', ['gnome-power-statistics', 'preferences-system-power']],
        ['org.gnome.Epiphany', 'globe', 'lagoon', ['epiphany', 'internet-web-browser', 'web-browser', 'org.gnome.Shell.CaptivePortal']],
        ['org.kde.krdc', 'remote', 'royal', ['krdc', 'virt-viewer', 'remote-viewer']],
        ['org.kde.skanpage', 'scanner', 'ice', ['skanlite', 'xsane']],
        ['abiword', 'document', 'ocean'],
        ['gnumeric', 'spreadsheet', 'emerald', ['org.gnome.Gnumeric']],
        ['preferences-desktop-display', 'computer', 'ocean', ['preferences-desktop-screensaver']],
        ['preferences-desktop-wallpaper', 'photo', 'sunset'],
        ['preferences-desktop-theme', 'palette', 'orchid'],
        ['preferences-desktop-locale', 'globe', 'emerald'],
    ],
    places: [
        ['folder', 'folder', 'sky', ['inode-directory', 'folder-open', 'folder-drag-accept', 'folder-visiting']],
        ['folder-documents', 'folder-documents', 'sky'],
        ['folder-download', 'folder-download', 'sky'],
        ['folder-music', 'folder-music', 'sky'],
        ['folder-pictures', 'folder-pictures', 'sky'],
        ['folder-videos', 'folder-videos', 'sky'],
        ['folder-templates', 'folder-templates', 'sky'],
        ['folder-publicshare', 'folder-publicshare', 'sky'],
        ['user-desktop', 'folder-desktop', 'sky', ['folder-desktop']],
        ['folder-remote', 'folder-remote', 'sky', ['folder-network']],
        ['folder-saved-search', 'search', 'sky'],
        ['folder-locked', 'folder-locked', 'sky'],
        ['folder-bookmarks', 'folder-bookmarks', 'sky'],
        // A folder given a colour in Files (metadata::custom-icon-name, /usr/libexec/amethystora-folder-colour): the
        // plain folder in that colour's ramp
        ...FOLDER_COLOURS.map(colour => [`folder-${colour}`, 'folder', `folder-${colour}`]),
        ['user-home', 'home', 'peach', ['folder-home']],
        ['user-trash', 'trash', 'ice'],
        ['user-trash-full', 'trash-full', 'ice'],
        ['network-server', 'server', 'indigo', ['network-workgroup']],
        ['user-bookmarks', 'bookmark', 'gold'],
        ['avatar-default', 'avatar', 'orchid', ['system-users', 'preferences-desktop-personal', 'goa-account']],
    ],
    mimetypes: [
        ['text-x-generic', 'text', 'blossom'],
        ['text-x-script', 'script', 'mint'],
        ['text-html', 'html', 'lagoon'],
        // Windows programs too, in the purple of every program: four squares on Microsoft's blue would read as its logo
        ['application-x-executable', 'app', 'royal', ['application-x-ms-dos-executable', 'application-x-msdownload', 'application-x-msi']],
        ['image-x-generic', 'image', 'sunset'],
        ['audio-x-generic', 'audio', 'orchid'],
        ['video-x-generic', 'video', 'grape'],
        ['package-x-generic', 'package', 'gold'],
        ['application-pdf', 'pdf', 'cherry'],
        ['x-office-document', 'document', 'ocean'],
        ['x-office-spreadsheet', 'spreadsheet', 'emerald'],
        ['x-office-presentation', 'presentation', 'coral'],
        ['x-office-drawing', 'drawing', 'magenta'],
        ['x-office-address-book', 'contact', 'coral', ['x-office-addressbook']],
        ['x-office-calendar', 'calendar', 'cherry', ['text-calendar']],
        ['font-x-generic', 'font', 'dusk'],
        ['text-x-generic-template', 'template', 'ice'],
        ['x-office-document-template', 'template', 'ocean'],
        ['x-office-spreadsheet-template', 'template', 'emerald'],
        ['x-office-presentation-template', 'template', 'coral'],
        ['x-office-drawing-template', 'template', 'magenta'],
        ['application-x-generic', 'blank', 'ice'],
        ['application-vnd.iccprofile', 'colour-profile', 'blossom'],
        ['inode-symlink', 'symlink', 'sky'],
        ['model', 'model-file', 'indigo'],
        ['application-x-addon', 'addon', 'lime'],
        ['application-x-firmware', 'chip', 'lime'],
        ['application-x-cd-image', 'disc', 'indigo', ['application-x-iso9660-image', 'application-x-raw-disk-image',
            'application-x-raw-disk-image-xz-compressed', 'application-x-qemu-disk', 'application-vnd.efi.iso', 'application-x-apple-diskimage']],
        ['application-zip', 'zip', 'gold', ['application-x-zip', 'application-x-tar', 'application-x-compressed-tar', 'application-x-bzip',
            'application-x-bzip-compressed-tar', 'application-gzip', 'application-x-gzip', 'application-x-xz', 'application-x-xz-compressed-tar',
            'application-x-7z-compressed', 'application-vnd.rar', 'application-x-rar', 'application-zstd', 'application-x-zstd-compressed-tar',
            'application-x-lzma', 'application-x-archive', 'application-x-cpio']],
        ['application-x-rpm', 'package', 'amber', ['application-vnd.debian.binary-package', 'application-vnd.flatpak', 'application-vnd.flatpak.ref',
            'application-vnd.flatpak.repo', 'application-x-java-archive', 'x-package-repository']],
        ['application-vnd.appimage', 'app', 'lagoon', ['application-x-sharedlib', 'application-x-desktop']],
        ['application-pgp-keys', 'key-file', 'gold', ['application-pgp-encrypted', 'application-pgp-signature', 'application-x-keepass2', 'application-x-pem-key']],
        ['application-x-x509-ca-cert', 'certificate', 'emerald', ['application-pkcs12', 'application-x-pkcs12', 'application-pkix-cert',
            'application-x-pkcs7-certificates', 'application-certificate']],
        ['application-x-bittorrent', 'download-file', 'emerald'],
        ['application-epub+zip', 'book-file', 'peach', ['application-x-mobipocket-ebook', 'application-vnd.comicbook+zip',
            'application-vnd.comicbook-rar', 'application-x-fictionbook+xml']],
        ['font-ttf', 'font', 'dusk', ['font-otf', 'font-woff', 'font-woff2', 'font-collection', 'application-x-font-ttf']],
        ['application-vnd.oasis.opendocument.text', 'document', 'ocean', ['application-vnd.openxmlformats-officedocument.wordprocessingml.document',
            'application-msword', 'application-rtf']],
        ['application-vnd.oasis.opendocument.spreadsheet', 'spreadsheet', 'emerald', ['application-vnd.openxmlformats-officedocument.spreadsheetml.sheet',
            'application-vnd.ms-excel']],
        ['application-vnd.oasis.opendocument.presentation', 'presentation', 'coral', ['application-vnd.openxmlformats-officedocument.presentationml.presentation',
            'application-vnd.ms-powerpoint']],
        ['application-vnd.oasis.opendocument.graphics', 'drawing', 'magenta'],
        ['image-png', 'image', 'sunset', ['image-jpeg', 'image-gif', 'image-webp', 'image-tiff', 'image-bmp', 'image-avif', 'image-heif', 'image-jxl', 'image-x-icon']],
        ['image-svg+xml', 'label-SVG', 'orchid'],
        ['audio-mpeg', 'audio', 'orchid', ['audio-flac', 'audio-x-wav', 'audio-ogg', 'audio-x-vorbis+ogg', 'audio-x-opus+ogg', 'audio-mp4', 'audio-aac', 'audio-x-m4a']],
        ['video-mp4', 'video', 'grape', ['video-x-matroska', 'video-webm', 'video-quicktime', 'video-x-msvideo', 'video-mpeg', 'video-ogg']],
        ['text-plain', 'text', 'blossom', ['application-x-subrip', 'text-vtt', 'text-x-readme', 'text-x-changelog', 'text-x-copying', 'text-x-log', 'text-x-preview']],
        ['message-rfc822', 'mail-file', 'lagoon', ['application-mbox']],
        ['text-vcard', 'contact', 'coral', ['text-x-vcard']],
        ['application-x-shellscript', 'script', 'mint', ['text-x-makefile', 'text-x-cmake', 'text-x-meson']],
        ['text-x-csrc', 'label-C', 'ocean', ['text-x-c']],
        ['text-x-chdr', 'label-H', 'ice'],
        ['text-x-c++src', 'label-C++', 'royal', ['text-x-c++hdr']],
        ['text-x-csharp', 'label-C#', 'grape'],
        ['text-x-go', 'label-GO', 'aqua'],
        ['text-rust', 'label-RS', 'coral', ['text-x-rust']],
        ['text-x-python', 'label-PY', 'amber', ['text-x-python3', 'application-x-python-bytecode']],
        ['application-javascript', 'label-JS', 'gold', ['text-javascript', 'application-x-javascript']],
        ['application-x-typescript', 'label-TS', 'lagoon', ['text-x-typescript']],
        ['text-css', 'label-CSS', 'sky'],
        ['application-x-php', 'label-PHP', 'indigo', ['text-x-php']],
        ['application-x-ruby', 'label-RB', 'cherry', ['text-x-ruby']],
        ['text-x-lua', 'label-LUA', 'royal'],
        ['text-x-kotlin', 'label-KT', 'dusk'],
        ['text-x-java', 'label-JAVA', 'sunset'],
        ['application-sql', 'label-SQL', 'emerald', ['text-x-sql']],
        ['text-markdown', 'label-MD', 'ice', ['text-x-markdown']],
        ['application-x-yaml', 'label-YML', 'lime', ['application-yaml', 'text-x-yaml']],
        ['application-toml', 'label-TOML', 'peach', ['text-x-toml']],
        ['application-xml', 'label-XML', 'flame', ['text-xml']],
        ['text-x-tex', 'label-TEX', 'mint'],
        ['text-x-haskell', 'label-HS', 'orchid'],
        ['application-x-perl', 'label-PL', 'blossom', ['text-x-perl']],
        ['text-csv', 'label-CSV', 'emerald'],
        ['application-octet-stream', 'label-01', 'indigo'],
        ['application-json', 'json', 'lime', ['application-x-ipynb+json', 'application-schema+json']],
        ['text-x-patch', 'diff', 'coral', ['text-x-diff']],
        ['application-x-sqlite3', 'database', 'emerald', ['application-vnd.sqlite3', 'application-x-sqlite2']],
        ['application-x-trash', 'trash', 'ice'],
    ],
    devices: [
        ['computer', 'computer', 'ocean', ['video-display']],
        ['computer-laptop', 'laptop', 'royal'],
        ['phone', 'phone', 'grape', ['smartphone', 'pda']],
        ['drive-harddisk', 'drive', 'indigo', ['drive-harddisk-system', 'drive-multidisk']],
        ['drive-removable-media', 'usb', 'gold', ['drive-harddisk-usb', 'media-removable']],
        ['media-flash', 'card', 'amber'],
        ['media-optical', 'disc', 'ice', ['drive-optical']],
        ['camera-photo', 'camera', 'indigo'],
        ['camera-web', 'webcam', 'lagoon'],
        ['printer', 'printer', 'coral'],
        ['scanner', 'scanner', 'ice'],
        ['input-keyboard', 'keyboard', 'dusk'],
        ['input-mouse', 'mouse', 'sky'],
        ['input-gaming', 'gamepad', 'flame'],
        ['input-tablet', 'tablet', 'orchid'],
        ['audio-headphones', 'headphones', 'magenta', ['audio-headset']],
        ['audio-speakers', 'speaker', 'sunset', ['audio-card']],
        ['audio-input-microphone', 'microphone', 'cherry', ['microphone']],
        ['network-wired', 'plug', 'mint', ['nm-device-wired', 'nm-device-wired-secure', 'ac-adapter']],
        ['modem', 'modem', 'aqua', ['ModemManager', 'nm-device-wwan']],
        ['media-tape', 'tape', 'peach'],
        ['input-dialpad', 'dialpad', 'royal'],
        ['ebook-reader', 'ereader', 'peach'],
        ['network-wireless', 'wifi', 'lagoon'],
        ['battery', 'battery', 'lime', ['uninterruptible-power-supply']],
        ['input-touchpad', 'touchpad', 'sky'],
        ['media-floppy', 'floppy', 'indigo'],
        ['multimedia-player', 'player', 'magenta'],
        ['camera-video', 'webcam', 'lagoon'],
        ['printer-network', 'printer', 'coral'],
    ],
    status: [
        ['dialog-information', 'info', 'sky', ['help-about']],
        ['dialog-warning', 'warning', 'amber'],
        ['dialog-error', 'error', 'cherry'],
        ['dialog-question', 'question', 'royal', ['help-faq', 'zenity']],
        ['dialog-password', 'key', 'gold'],
        ['security-high', 'shield-ok', 'emerald'],
        ['security-medium', 'shield-alert', 'amber'],
        ['security-low', 'shield-alert', 'cherry'],
        ['image-missing', 'image-missing', 'cherry'],
        ['image-loading', 'image-loading', 'sky'],
    ],
    categories: [
        ['applications-system', 'settings', 'ocean', ['preferences-other']],
        ['applications-engineering', 'chip', 'aqua'],
        ['applications-utilities', 'wrench', 'lime', ['applications-accessories']],
        ['applications-multimedia', 'play', 'grape'],
        ['applications-graphics', 'palette', 'blossom'],
        ['applications-internet', 'globe', 'lagoon'],
        ['applications-office', 'document', 'ocean'],
        ['applications-development', 'code', 'royal'],
        ['applications-games', 'gamepad', 'flame'],
        ['applications-science', 'flask', 'aqua'],
        ['applications-education', 'cap', 'peach'],
        ['applications-other', 'app', 'royal'],
        ['applications-fonts', 'letter', 'dusk'],
    ],
};

// Apps whose icon is their maker's mark (a logo, a mascot, a letter of theirs) keep the one they ship: redrawn in the
// packs' style it would be a changed trademark, and drawn as a pictogram it would no longer say whose app it is. The
// packs refuse to draw any of these names
const LOGOS = /^(firefox|org\.mozilla\..*|thunderbird|waydroid|io\.podman_desktop\..*|steam|com\.valvesoftware\..*|codium|vscodium|com\.vscodium\..*|code|com\.visualstudio\..*|google-chrome|com\.google\..*|chromium.*|org\.chromium\..*|brave.*|com\.brave\..*|com\.microsoft\..*|discord|com\.discordapp\..*|spotify.*|com\.spotify\..*|telegram.*|org\.telegram\..*|signal.*|org\.signal\..*|slack|com\.slack\..*|zoom|us\.zoom\..*|obsidian|md\.obsidian\..*|org\.flathub\..*|bottles|com\.usebottles\..*|io\.github\.flattool\.Ignition|libreoffice.*|org\.libreoffice\..*|vlc|org\.videolan\..*|obs|com\.obsproject\..*|blender|org\.blender\..*|gimp|org\.gimp\..*|inkscape|org\.inkscape\..*|krita|org\.kde\.krita|kdenlive|org\.kde\.kdenlive|kate|org\.kde\.kate|audacity|org\.audacityteam\..*|net\.lutris\..*|com\.heroicgameslauncher\..*|kitty|alacritty|org\.wezfurlong\..*|com\.mitchellh\..*|org\.qbittorrent\..*|org\.mozilla|com\.jetbrains\..*|jetbrains-.*|net\.opentabletdriver\..*|solaar|org\.fedoraproject\..*|fedora.*|goa-account-.+|org\.wireshark\..*|wireshark|qemu|com\.anydesk\..*|anydesk|com\.bitwarden\..*|bitwarden|com\.dropbox\..*|dropbox|com\.mojang\..*|minecraft|com\.nextcloud\..*|nextcloud|com\.opera\..*|opera|com\.vivaldi\..*|vivaldi.*|com\.rustdesk\..*|rustdesk|org\.darktable\..*|darktable|org\.filezillaproject\..*|filezilla|fr\.handbrake\..*|org\.keepassxc\..*|keepassxc|org\.onlyoffice\..*|onlyoffice.*|org\.prismlauncher\..*|org\.shotcut\..*|shotcut)$/i;

const CONTEXTS = {apps: 'Applications', places: 'Places', mimetypes: 'MimeTypes', devices: 'Devices', status: 'Status', categories: 'Categories'};

const mixWhite = (hex, s) => '#' + [1, 3, 5].map(i => Math.round(parseInt(hex.slice(i, i + 2), 16) * (1 - s) + 255 * s)
    .toString(16).padStart(2, '0')).join('');
const gradient = (id, colours) => `<linearGradient id="${id}" x1="0" y1="0" x2="1" y2="1">${
    colours.map((c, i) => `<stop offset="${i / (colours.length - 1)}" stop-color="${c}"/>`).join('')}</linearGradient>`;
const shadow = (id, colour, opacity, dy, blur) => `<filter id="${id}" x="-25%" y="-25%" width="150%" height="150%">` +
    `<feDropShadow dx="0" dy="${dy}" stdDeviation="${blur}" flood-color="${colour}" flood-opacity="${opacity}"/></filter>`;
const svg = (defs, body) => `<svg xmlns="http://www.w3.org/2000/svg" width="128" height="128" viewBox="0 0 256 256"><defs>${defs}</defs>${body}</svg>\n`;

// A colour moved in OKLab lightness (its hue kept, its chroma eased where the gamut clips) until its luminance, as WCAG
// computes it, lies in [lo, hi]: the packs for a ground that is not dark use it to deepen the ramps, and Line to hold
// every colour in the band that reads on all ten themes' grounds
const toLinear = v => (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);
const toSrgb = v => (v <= 0.0031308 ? 12.92 * v : 1.055 * v ** (1 / 2.4) - 0.055);
const channels = hex => [1, 3, 5].map(i => parseInt(hex.slice(i, i + 2), 16) / 255);
const luminance = hex => { const [r, g, b] = channels(hex).map(toLinear); return 0.2126 * r + 0.7152 * g + 0.0722 * b; };
function toOklab(hex) {
    const [r, g, b] = channels(hex).map(toLinear);
    const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
    const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
    const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
    return [0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s, 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
        0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s];
}
function fromOklab([L, a, b]) {
    const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3, m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3;
    const s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
    return '#' + [4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s, -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
        -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s].map(v => Math.round(Math.min(1, Math.max(0, toSrgb(v))) * 255)
        .toString(16).padStart(2, '0')).join('');
}
function band(hex, lo, hi) {
    let [L, a, b] = toOklab(hex);
    for (let i = 0; i < 60; i++) {
        const y = luminance(fromOklab([L, a, b]));
        if (y < lo) L += 0.01; else if (y > hi) L -= 0.01; else break;
        a *= 0.995; b *= 0.995;
    }
    return fromOklab([L, a, b]);
}
// Chroma taken down and lightness a touch, for the quiet pack
const mute = (hex, k, dl = -0.04) => { const [L, a, b] = toOklab(hex); return fromOklab([L + dl, a * k, b * k]); };
const mapRamp = (ramp, f) => ramp.map(stops => stops.map(f));

// How each pack draws a glyph in a ramp: Glass fills it with the gradient and its details with a lighter one, under a
// glass sheen and a shadow; Neon lights the outline and the details in the prism; Duotone fills it with the gradient and
// draws its details in the contrasting one. The other packs are these, or a new drawing, in other colours.
const GLASS = {white: 0.55, shade: '#1a0b2e', opacity: 0.55, dy: 10, blur: 9, sheen: 0.55};
const drawGlass = o => (glyph, [fill]) => {
    const soft = fill.map(c => mixWhite(c, o.white));
    return svg(gradient('g', fill) + gradient('h', soft) +
        '<linearGradient id="s" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#fff"/><stop offset="0.55" stop-color="#fff" stop-opacity="0"/></linearGradient>' +
        `<mask id="m">${glyph({ink: '#fff', soft: '#fff'})}</mask>` + shadow('f', o.shade, o.opacity, o.dy, o.blur),
    `<g filter="url(#f)">${glyph({ink: 'url(#g)', soft: 'url(#h)'})}</g>` +
        `<rect width="256" height="150" fill="url(#s)" fill-opacity="${o.sheen}" mask="url(#m)"/>`);
};
const drawNeon = (glyph, [, prism, detail]) => {
    // What the glyph draws in its third ink, `over` (glyphs/glyphs.mjs): the shapes in the body's ink on top of a
    // soft one, which the soft layer would paint over. They are lit last, in the details' colour
    const lit = glyph({ink: 'none', soft: 'none', over: 'url(#d)'}), over = lit.includes('url(#d)') ? lit : '';
    return svg(gradient('g', prism) + (over ? gradient('d', detail) : '') +
        // The body's outline: its alpha eroded and taken from it leaves a ring, blurred twice under itself
        '<filter id="n" x="-25%" y="-25%" width="150%" height="150%"><feMorphology in="SourceAlpha" operator="erode" radius="6" result="inner"/>' +
        '<feComposite in="SourceGraphic" in2="inner" operator="out" result="ring"/><feGaussianBlur in="ring" stdDeviation="5" result="glow"/>' +
        '<feMerge><feMergeNode in="glow"/><feMergeNode in="glow"/><feMergeNode in="ring"/></feMerge></filter>' +
        '<filter id="l" x="-25%" y="-25%" width="150%" height="150%"><feGaussianBlur stdDeviation="4" result="glow"/>' +
        '<feMerge><feMergeNode in="glow"/><feMergeNode in="SourceGraphic"/></feMerge></filter>',
    `<g filter="url(#n)">${glyph({ink: 'url(#g)', soft: 'none', over: 'none'})}</g><g filter="url(#l)">${glyph({ink: 'none', soft: 'url(#g)', over: 'none'})}</g>` +
        (over && `<g filter="url(#l)">${over}</g>`));
};
const drawDuotone = (glyph, [fill, , contrast]) => svg(gradient('g', fill) + gradient('h', contrast) + shadow('f', fill[1], 0.45, 6, 7),
    `<g filter="url(#f)">${glyph({ink: 'url(#g)', soft: 'url(#h)'})}</g>`);

// Every pack: its name, what it is, how its ramps are changed for it (the glyph's own gradients when it says nothing),
// and how it draws. Each theme names the pack that suits its colours and its style (themes/*/icons.theme).
const PACKS = {
    glass: {
        name: 'Amethystora Glass',
        comment: 'Amethystora\'s own icons, each in its own gradient with a glass sheen',
        draw: drawGlass(GLASS),
    },
    'glass-light': {
        name: 'Amethystora Glass for light',
        comment: 'Glass with every colour deepened until it reads on a light ground',
        // Luminance 0.06 to 0.24 is 3:1 or better against the lightest ground of the three light themes
        ramp: ramp => mapRamp(ramp, hex => band(hex, 0.06, 0.24)),
        draw: drawGlass(GLASS),
    },
    neon: {
        name: 'Amethystora Neon',
        comment: 'Amethystora\'s own icons, their outlines lit in colours of their own',
        draw: drawNeon,
    },
    'neon-light': {
        name: 'Amethystora Neon for light',
        comment: 'Neon with every colour deepened until it reads on a light ground',
        ramp: ramp => mapRamp(ramp, hex => band(hex, 0.06, 0.24)),
        draw: drawNeon,
    },
    duotone: {
        name: 'Amethystora Duotone',
        comment: 'Amethystora\'s own icons, each in its own gradient with its details in a contrasting one',
        draw: drawDuotone,
    },
    line: {
        name: 'Amethystora Line',
        comment: 'Amethystora\'s own icons as one even outline, in colours that read on every theme',
        // Luminance 0.2 to 0.25 is 3:1 or better against all ten themes' grounds, the lightest and the darkest
        ramp: ramp => mapRamp(ramp, hex => band(hex, 0.2, 0.25)),
        // The body's outline as an even ring and the details solid, in the same gradient: no fill, no glow
        draw: (glyph, [fill]) => svg(gradient('g', fill) +
            '<filter id="r" x="-10%" y="-10%" width="120%" height="120%"><feMorphology in="SourceAlpha" operator="erode" radius="9" result="in"/>' +
            '<feComposite in="SourceGraphic" in2="in" operator="out"/></filter>',
        `<g filter="url(#r)">${glyph({ink: 'url(#g)', soft: 'none', over: 'none'})}</g>` +
            `<g>${glyph({ink: 'none', soft: 'url(#g)', over: 'url(#g)'})}</g>`),
    },
    clay: {
        name: 'Amethystora Clay',
        comment: 'Amethystora\'s own icons as soft matte volumes, a highlight above and a shade below',
        ramp: ramp => mapRamp(ramp, hex => band(hex, 0.13, 0.26)),
        // A highlight from the top left and a shade at the bottom right cut from the body's own alpha, a soft shadow
        // under it, and the details in a pale tint of the body
        draw: (glyph, [fill]) => svg(gradient('g', fill) + gradient('h', fill.map(c => mixWhite(c, 0.78))) +
            '<filter id="c" x="-25%" y="-25%" width="150%" height="150%">' +
            '<feGaussianBlur in="SourceAlpha" stdDeviation="5" result="b"/>' +
            '<feOffset in="b" dx="5" dy="7" result="o1"/><feComposite in="SourceAlpha" in2="o1" operator="out" result="hi"/>' +
            '<feFlood flood-color="#ffffff" flood-opacity="0.55"/><feComposite in2="hi" operator="in" result="hiC"/>' +
            '<feOffset in="b" dx="-5" dy="-7" result="o2"/><feComposite in="SourceAlpha" in2="o2" operator="out" result="sh"/>' +
            '<feFlood flood-color="#1b1030" flood-opacity="0.32"/><feComposite in2="sh" operator="in" result="shC"/>' +
            '<feMerge result="lit"><feMergeNode in="SourceGraphic"/><feMergeNode in="hiC"/><feMergeNode in="shC"/></feMerge>' +
            '<feComposite in="lit" in2="SourceAlpha" operator="in" result="body"/>' +
            '<feGaussianBlur in="SourceAlpha" stdDeviation="5" result="d"/><feOffset in="d" dy="6" result="dd"/>' +
            '<feFlood flood-color="#1b1030" flood-opacity="0.2"/><feComposite in2="dd" operator="in" result="drop"/>' +
            '<feMerge><feMergeNode in="drop"/><feMergeNode in="body"/></feMerge></filter>',
        `<g filter="url(#c)">${glyph({ink: 'url(#g)', soft: 'url(#h)'})}</g>`),
    },
    muted: {
        name: 'Amethystora Muted',
        comment: 'Glass in quieter colours with half the sheen, for the themes of low chroma',
        // A folder colour keeps more of its chroma: telling folders apart is all it is for, and at the others' strength
        // Sky, Slate and Amethyst run together
        ramp: ramp => {
            const [fill, prism, contrast] = ramp, k = FOLDER_RAMPS.has(ramp) ? 0.85 : 0.42;
            return [fill.map(hex => band(mute(hex, k), 0.22, 0.75)), prism.map(hex => band(mute(hex, k), 0.22, 0.75)),
                contrast.map(hex => mute(hex, 0.5, 0.02))];
        },
        draw: drawGlass({white: 0.5, shade: '#0b0f14', opacity: 0.4, dy: 8, blur: 8, sheen: 0.25}),
    },
};
const drawn = (id, glyph, ramp) => PACKS[id].draw(DRAW[glyph], (PACKS[id].ramp ?? (r => r))(RAMPS[ramp]));

// The folder colours must stay told apart in every pack: the middle of what each pack draws them in (the mean of the
// fill's stops, of the prism's for Neon), in OKLab, at least 0.08 from every other
for (const [id, pack] of Object.entries(PACKS)) {
    const middle = colour => {
        const [fill, prism] = (pack.ramp ?? (r => r))(RAMPS[`folder-${colour}`]);
        const stops = (pack.draw === drawNeon ? prism : fill).map(toOklab);
        return stops[0].map((_, i) => stops.reduce((sum, stop) => sum + stop[i], 0) / stops.length);
    };
    const points = FOLDER_COLOURS.map(middle);
    FOLDER_COLOURS.forEach((one, i) => FOLDER_COLOURS.slice(i + 1).forEach((other, k) => {
        const distance = Math.hypot(...points[i].map((v, j) => v - points[i + 1 + k][j]));
        if (distance < 0.08)
            throw new Error(`${id}: the folder colours ${one} and ${other} are ${distance.toFixed(3)} apart in OKLab, under 0.08`);
    }));
}

function indexTheme(pack) {
    const dirs = Object.keys(SET).map(context => `${context}/scalable`);
    return `[Icon Theme]\nName=${pack.name}\nComment=${pack.comment}\nInherits=Adwaita,hicolor\nExample=folder\n` +
        `Directories=${dirs.join(',')}\n` + Object.entries(CONTEXTS).map(([dir, context]) =>
        `\n[${dir}/scalable]\nContext=${context}\nSize=128\nMinSize=8\nMaxSize=512\nType=Scalable\n`).join('');
}

// A name is drawn once, and never one of a maker's mark
const names = Object.values(SET).flat().flatMap(([name, , , also = []]) => [name, ...also]);
for (const name of names) {
    if (LOGOS.test(name))
        throw new Error(`${name}: its icon is its maker's mark, which the packs leave as it ships`);
    if (names.indexOf(name) !== names.lastIndexOf(name))
        throw new Error(`${name}: drawn twice`);
}

// Each name an icon answers to is a file of its own rather than a link, since the image build's rsync of
// system_files leaves links out
for (const [id, pack] of Object.entries(PACKS)) {
    const dir = join(ICONS, `amethystora-${id}`);
    rmSync(dir, {recursive: true, force: true});
    for (const [context, icons] of Object.entries(SET)) {
        mkdirSync(join(dir, context, 'scalable'), {recursive: true});
        for (const [name, glyph, ramp, also = []] of icons) {
            if (!DRAW[glyph] || !RAMPS[ramp])
                throw new Error(`${name}: no glyph ${glyph} or ramp ${ramp}`);
            const file = drawn(id, glyph, ramp);
            for (const each of [name, ...also])
                writeFileSync(join(dir, context, 'scalable', `${each}.svg`), file);
        }
    }
    writeFileSync(join(dir, 'index.theme'), indexTheme(pack));
    console.log(`wrote ${dir}`);
}

// The two launchers of Amethystora's own that have no artwork of theirs in hicolor (branding/generate.mjs draws
// the others), in the Glass style: every icon theme falls back on hicolor, and the Security app draws its shield
// from this file as it runs
for (const [name, glyph, ramp] of SET.apps.filter(([name]) => ['amethystora-security-status', 'amethystora-notes'].includes(name))) {
    mkdirSync(join(ICONS, 'hicolor/scalable/apps'), {recursive: true});
    writeFileSync(join(ICONS, 'hicolor/scalable/apps', `${name}.svg`), drawn('glass', glyph, ramp));
}

if (process.argv.includes('--preview')) {
    const out = join(tmpdir(), 'amethystora-icons');
    mkdirSync(out, {recursive: true});
    const all = Object.values(SET).flat();
    const rows = (id, size) => all.map(([name, glyph, ramp]) =>
        `<img title="${name}" width="${size}" height="${size}" src="data:image/svg+xml;base64,${Buffer.from(drawn(id, glyph, ramp)).toString('base64')}">`).join('');
    const ground = (bg, ink) => Object.keys(PACKS).map(id =>
        `<section style="background:${bg};color:${ink};padding:18px 24px"><h2 style="margin:0 0 10px;font:600 18px sans-serif">${PACKS[id].name}</h2>` +
        `<div style="display:flex;flex-wrap:wrap;gap:10px">${rows(id, 64)}</div>` +
        `<div style="display:flex;flex-wrap:wrap;gap:6px;margin-top:12px">${rows(id, 24)}</div></section>`).join('');
    writeFileSync(join(out, 'sheet.html'), `<!doctype html><meta charset="utf-8"><body style="margin:0;width:1500px">` +
        ground('radial-gradient(ellipse at 20% 0%, #3a1470 0%, #16042c 55%, #0b0418 100%)', '#e8e0f2') +
        ground('linear-gradient(180deg,#fdf8ff,#e9e1f5)', '#3d3450') + '</body>');
    console.log(join(out, 'sheet.html'));
}
