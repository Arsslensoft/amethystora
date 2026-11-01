#!/usr/bin/bash
# vmlab-vm: r5-apps
# Rounds 5 and 6's apps group (plans/ui-ideas/spec-apps.md, section 8), in the test account's session, asked of GNOME
# Shell itself through Eval, which a lab-only extension of the test account opens (never in the image):
#   - the appgrid schema's new keys and the backup disk's, as the image ships them;
#   - an app hidden from the grid, its folders and the search, and shown again; still pinnable in the dock;
#   - an app renamed (its label, get_name(), the forgiving search) and its own name back;
#   - an app's icon from another pack, and its own back; a picture made into an icon by amethystora-app-look;
#   - the grid in order (A to Z after the folders), and the account's own order back unchanged;
#   - the app menu's items in the grid, What can it reach? on a Flatpak only, and Security opened on that app;
#   - what starts with you: an entry listed, turned off with exactly the Hidden shape, on again as it was; the image's
#     own not switched;
#   - the templates seeded and not executable; Downloads tidied and put back;
#   - the backup disk recognized on a mount at the repository's place;
#   - ten disables and enables leave GNOME's own grid, names and filter.
# vmlab-paths: @ui/extension.js @ui/modules/appgrid* @ui/modules/fuzzy.js @ui/modules/bazaar.js @ui/modules/backupdisk.js @ui/stylesheets/appgrid.css @ui/schemas/*appgrid* @ui/schemas/*backupdisk* @sf/usr/libexec/amethystora-app-look @sf/usr/libexec/amethystora-startup @sf/usr/libexec/amethystora-tidy-downloads @sf/usr/lib/systemd/user/amethystora-tidy-downloads.* @sf/*28-templates.sh @sf/usr/libexec/amethystora-security-watch @sf/usr/libexec/amethystora-control-list @sf/usr/libexec/amethystora-setup* @sf/*just/desktop.just @sf/*just/apps.just apps/security/* apps/control/* apps/control-core/*
source "$(dirname "$0")/lib.sh"

UUID=amethystora-ui@arsslensoft.github.io
HOME_DIR="/var/home/${TEST_USER}"
HERE="$(dirname "$(readlink -f "$0")")"
WORK=/run/vmlab-61
APPGRID=org.gnome.shell.extensions.amethystora-ui.appgrid
BACKUPDISK=org.gnome.shell.extensions.amethystora-ui.backupdisk
ev() {
    user_bus gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.gnome.Shell.Eval "$1" |
        python3 -c 'import json, sys
from gi.repository import GLib
v = GLib.Variant.parse(None, sys.stdin.read().strip(), None, None)
try:
    x = json.loads(v[1])
except ValueError:
    x = v[1]
print(x if isinstance(x, str) else json.dumps(x))'
}
gs() { in_session "gsettings $*" 2>/dev/null; }
same() { if [[ "$2" == "$3" ]]; then ok "$1"; else not_ok "$1" "expected: $3" "found: $2"; fi; }
UI="Main.extensionManager.lookup('${UUID}').stateObj.modules"
GRID="Main.overview._overview.controls._appDisplay"
LOOKS="${UI}.get('appgrid')._looks"

if ! has_session; then
    bash "$(dirname "$0")/10-account.sh" >/dev/null
fi
if ! wait_for 300 has_session; then
    not_ok "no session for ${TEST_USER}"
    finish
fi
install -d -o "${TEST_USER}" -g "${TEST_USER}" "${HOME_DIR}/.local/share/gnome-shell/extensions/vmlab-eval@lab"
cat >"${HOME_DIR}/.local/share/gnome-shell/extensions/vmlab-eval@lab/metadata.json" <<'EOF'
{"uuid": "vmlab-eval@lab", "name": "VM lab", "description": "The lab's Eval", "shell-version": ["49", "50", "51"]}
EOF
cat >"${HOME_DIR}/.local/share/gnome-shell/extensions/vmlab-eval@lab/extension.js" <<'EOF'
export default class { enable() { global.context.unsafe_mode = true; } disable() {} }
EOF
install -d -m 0755 -o "${TEST_USER}" -g "${TEST_USER}" "${WORK}"
cp "${HERE}/atspi.py" "${WORK}/"
chown -R "${TEST_USER}:" "${HOME_DIR}/.local/share/gnome-shell" "${WORK}"
enabled="$(user_bus gsettings get org.gnome.shell enabled-extensions)"
if [[ ${enabled} != *vmlab-eval@lab* ]]; then
    user_bus gsettings set org.gnome.shell enabled-extensions "${enabled%]*}, 'vmlab-eval@lab']"
    loginctl unlock-sessions
    systemctl restart gdm
fi
looks_ready() { [[ "$(ev "typeof ${LOOKS}?.hide")" == function ]]; }
if ! wait_for 240 has_session || ! wait_for 120 looks_ready; then
    not_ok "the app grid's looks did not start in the session"
    finish
fi
# GNOME Shell's own modules, for the checks below (Eval runs no import of its own)
ev "(async () => { globalThis.vm61 = {AD: await import('resource:///org/gnome/shell/ui/appDisplay.js'),
    Dash: await import('resource:///org/gnome/shell/ui/dash.js'),
    Fav: await import('resource:///org/gnome/shell/ui/appFavorites.js'),
    PCM: await import('resource:///org/gnome/shell/misc/parentalControlsManager.js')}; })(); 'importing'" >/dev/null
imported() { [[ "$(ev "typeof globalThis.vm61?.PCM?.getDefault")" == function ]]; }
wait_for 20 imported || not_ok "GNOME Shell's modules could not be imported in Eval"
for key in order hidden names icons; do gs reset "${APPGRID}" "${key}"; done
gs reset "${BACKUPDISK}" ask
LAYOUT_BEFORE="$(gs get org.gnome.shell app-picker-layout)"
cleanup() {
    for key in order hidden names icons; do gs reset "${APPGRID}" "${key}"; done
    gs reset "${BACKUPDISK}" ask
    rm -rf "${HOME_DIR}/.config/autostart/vmlab-61-start.desktop" "${HOME_DIR}/.local/share/amethystora/startup-off" \
        "${HOME_DIR}/.local/share/amethystora/app-icons" "${WORK}"
    ev "Main.overview.hide(); 'hidden'" >/dev/null
}
trap cleanup EXIT

# An app of the grid's own (not pinned, so GNOME 51 shows it in the grid), with a drawing in the Neon pack, and one
# installed as a Flatpak, if the machine has one
pick() {
    ev "(() => {
        const favs = global.settings.get_strv('favorite-apps');
        const apps = imports.gi.Shell.AppSystem.get_default().get_installed()
            .filter(i => i.should_show() && !favs.includes(i.get_id()));
        const drawn = i => { const icon = i.get_icon(); return icon && icon.get_names &&
            icon.get_names().some(n => imports.gi.GLib.file_test('/usr/share/icons/amethystora-neon/apps/scalable/' + n + '.svg',
                imports.gi.GLib.FileTest.EXISTS)); };
        const one = apps.find(i => drawn(i) && !i.get_filename().includes('/flatpak/')) ?? apps[0];
        const flatpak = apps.find(i => i.get_filename().includes('/flatpak/exports/'));
        return JSON.stringify([one?.get_id() ?? '', one?.get_name() ?? '', flatpak?.get_id() ?? '', drawn(one) ? 'yes' : 'no']);
    })()" | jq -r '.[]'
}
mapfile -t PICKED < <(pick)
APP="${PICKED[0]:-}" OWN="${PICKED[1]:-}" FLATPAK="${PICKED[2]:-}" DRAWN="${PICKED[3]:-no}"
if [[ -z "${APP}" ]]; then
    not_ok "no app to try the grid's looks with"
    finish
fi
ev "Main.overview.showApps(); 'shown'" >/dev/null
sleep 3

# --- 1: the keys as the image ships them -----------------------------------------------------------------------------
same "1 the app grid's keys" "$(GSETTINGS_BACKEND=memory gsettings list-keys "${APPGRID}" | sort | paste -sd ' ')" \
    "hidden icons names order recent"
same "1 nothing hidden, renamed or sorted" "$(gs get "${APPGRID}" order) $(gs get "${APPGRID}" hidden) $(gs get "${APPGRID}" names)" \
    "'custom' @as [] @a{ss} {}"
same "1 the backup disk is recognized as the image ships" "$(gs get "${BACKUPDISK}" ask)" true

# --- 2: hidden from the grid, its folders and the search, and back ----------------------------------------------------
# Where an app's icon is: the grid's top level, a folder, or nowhere
in_grid() {
    ev "(() => { const g = ${GRID}; const id = '$1';
        if (g._orderedItems.some(i => i.id === id)) return 'top';
        if (g._folderIcons.some(f => f.getAppIds().includes(id))) return 'folder';
        return 'none'; })()"
}
# What the overview's app search (GNOME's, with the forgiving search's ranking over it) answers for a word
in_search() {
    ev "(() => { globalThis.vm61found = 'pending';
        const p = Main.overview.searchController._searchResults._providers.find(x => x.id === 'applications');
        p.getInitialResultSet(['$1'.toLowerCase()], null).then(ids => { globalThis.vm61found = ids.includes('$2') ? 'found' : 'not found'; });
        return 'asked'; })()" >/dev/null
    searched() { [[ "$(ev "globalThis.vm61found")" != pending ]]; }
    wait_for 10 searched
    ev "globalThis.vm61found"
}
WHERE_BEFORE="$(in_grid "${APP}")"
SEARCH_WORD="$(cut -d' ' -f1 <<<"${OWN}" | tr -cd '[:alnum:]')"
same "2 the app is in the grid before" "$([[ "${WHERE_BEFORE}" != none ]] && echo yes)" yes
same "2 and the search finds it" "$(in_search "${SEARCH_WORD}" "${APP}")" found
in_session "ame desktop app hide ${APP}" >/dev/null
gone() { [[ "$(in_grid "${APP}")" == none ]]; }
if wait_for 10 gone; then ok "2 hidden: not in the grid or its folders"; else not_ok "2 hidden: not in the grid or its folders" "$(in_grid "${APP}")"; fi
same "2 hidden: not in the search" "$(in_search "${SEARCH_WORD}" "${APP}")" "not found"
same "2 no launcher was written for it" "$(ls "${HOME_DIR}/.local/share/applications/${APP}" 2>/dev/null)" ""
same "2 a hidden app can still be pinned" \
    "$(ev "(() => { const f = vm61.Fav.getAppFavorites(); f.addFavorite('${APP}'); const r = f.isFavorite('${APP}'); f.removeFavorite('${APP}'); return r; })()")" true
in_session "ame desktop app show ${APP}" >/dev/null
back() { [[ "$(in_grid "${APP}")" == "${WHERE_BEFORE}" ]]; }
if wait_for 10 back; then ok "2 shown again, where it was"; else not_ok "2 shown again, where it was" "$(in_grid "${APP}")"; fi

# --- 3: renamed, and its own name back -------------------------------------------------------------------------------
label() { ev "(() => { let found = ''; const walk = a => { if (a instanceof vm61.AD.AppIcon && a.app?.get_id() === '${APP}') found = a.icon.label?.text ?? ''; for (const c of a.get_children()) walk(c); }; walk(Main.layoutManager.overviewGroup); return found; })()"; }
in_session "ame desktop app rename ${APP} 'Lab Sixty One'" >/dev/null
renamed() { [[ "$(ev "imports.gi.Shell.AppSystem.get_default().lookup_app('${APP}').get_name()")" == "Lab Sixty One" ]]; }
if wait_for 10 renamed; then ok "3 renamed: its name"; else not_ok "3 renamed: its name"; fi
if [[ "${WHERE_BEFORE}" == top ]]; then
    same "3 renamed: its label in the grid" "$(label)" "Lab Sixty One"
else
    skip "3 renamed: its label in the grid (the app is in a folder here)"
fi
same "3 renamed: the search finds the new name" "$(in_search "Sixty" "${APP}")" found
same "3 the name is only a key" "$(gs get "${APPGRID}" names)" "{'${APP}': 'Lab Sixty One'}"
in_session "ame desktop app rename ${APP}" >/dev/null
same "3 its own name back" "$(ev "imports.gi.Shell.AppSystem.get_default().lookup_app('${APP}').get_name()")" "${OWN}"

# --- 4: an icon from another pack, a picture, and its own back ---------------------------------------------------------
icon_file() { ev "(() => { const t = imports.gi.Shell.AppSystem.get_default().lookup_app('${APP}').create_icon_texture(64); return t.gicon?.get_file?.()?.get_path() ?? 'own'; })()"; }
OWN_ICON="$(icon_file)"
if [[ "${DRAWN}" == yes ]]; then
    in_session "ame desktop app icon ${APP} amethystora-neon" >/dev/null
    same "4 an icon from the Neon pack" "$(icon_file | grep -c '^/usr/share/icons/amethystora-neon/apps/scalable/')" 1
else
    skip "4 an icon from the Neon pack (no app here that it draws)"
fi
in_session "ame desktop app icon ${APP} amethystora-nonesuch" >/dev/null 2>&1 && not_ok "4 a pack that is not there was taken" ||
    ok "4 a pack that is not there is refused"
python3 - "${WORK}/picture.png" <<'EOF'
import sys
import gi
gi.require_version("GdkPixbuf", "2.0")
from gi.repository import GdkPixbuf
p = GdkPixbuf.Pixbuf.new(GdkPixbuf.Colorspace.RGB, True, 8, 512, 300)
p.fill(0x9b59b6ff)
p.savev(sys.argv[1], "png", [], [])
EOF
chown "${TEST_USER}:" "${WORK}/picture.png"
in_session "ame desktop app icon ${APP} picture ${WORK}/picture.png" >/dev/null
picture="$(icon_file)"
same "4 a picture made into an icon of the account's own" \
    "$([[ "${picture}" == "${HOME_DIR}/.local/share/amethystora/app-icons/"*.png ]] && echo yes)" yes
same "4 the picture was written again at 256 pixels" \
    "$(python3 -c 'import sys, gi; gi.require_version("GdkPixbuf", "2.0"); from gi.repository import GdkPixbuf; p = GdkPixbuf.Pixbuf.new_from_file(sys.argv[1]); print(max(p.get_width(), p.get_height()))' "${picture}" 2>/dev/null)" 256
in_session "ame desktop app icon ${APP} reset" >/dev/null
same "4 its own icon back" "$(icon_file)" "${OWN_ICON}"
same "4 and its pictures gone" "$(find "${HOME_DIR}/.local/share/amethystora/app-icons" -name '*.png' 2>/dev/null | wc -l)" 0

# --- 5: the grid in order, and the account's own order back --------------------------------------------------------------
top_level() { ev "JSON.stringify(${GRID}._orderedItems.filter(i => i.visible).map(i => [i instanceof vm61.AD.FolderIcon ? 'f' : 'a', i.app?.get_name() ?? i.name]))"; }
# (hiding an app above gave up its place, as an app uninstalled does: the layout is read again here)
LAYOUT_BEFORE="$(gs get org.gnome.shell app-picker-layout)"
in_session "ame desktop app-grid order name" >/dev/null
sleep 2
ORDERED="$(top_level)"
same "5 A to Z: the folders first" "$(jq -r '[.[].[0]] | join("")' <<<"${ORDERED}" | grep -cE '^f*a*$')" 1
same "5 A to Z: then the apps by name" \
    "$(jq -r '[.[] | select(.[0] == "a") | .[1]] | . == (sort_by(ascii_downcase))' <<<"${ORDERED}")" true
same "5 the account's own layout is not written while sorted" "$(gs get org.gnome.shell app-picker-layout)" "${LAYOUT_BEFORE}"
in_session "ame desktop app-grid order kind" >/dev/null
in_session "ame desktop app-grid order use" >/dev/null
in_session "ame desktop app-grid order custom" >/dev/null
sleep 2
same "5 its own order back, and its layout untouched" "$(gs get org.gnome.shell app-picker-layout)" "${LAYOUT_BEFORE}"

# --- 6-7: the menu, What can it reach?, and Security opened on that app --------------------------------------------------
menu_of() {
    ev "(() => { let icon = null; const walk = a => { if (!icon && a instanceof vm61.AD.AppIcon && !(a instanceof vm61.Dash.DashIcon) && a.app?.get_id() === '$1') icon = a; for (const c of a.get_children()) walk(c); }; walk(Main.layoutManager.overviewGroup);
        if (!icon) { const app = imports.gi.Shell.AppSystem.get_default().lookup_app('$1'); icon = new vm61.AD.AppIcon(app, {}); Main.layoutManager.overviewGroup.add_child(icon); global._vmlab61icon = icon; }
        icon.popupMenu(); const m = icon._menu; const items = m._amethystoraLooks;
        const r = items ? [items.section.visible, items.rename.label.text, items.icon.label.text, items.hide.label.text, items.reach.visible] : [];
        m.close(); global._vmlab61icon?.destroy(); global._vmlab61icon = null; return JSON.stringify(r); })()"
}
MENU="$(menu_of "${APP}")"
same "6 the grid's menu has Rename, Change icon and Hide" "$(jq -c '.[0:4]' <<<"${MENU}")" \
    '[true,"Rename…","Change icon…","Hide from the grid and search"]'
same "6 What can it reach? only on a Flatpak" "$(jq -c '.[4]' <<<"${MENU}")" \
    "$(ev "imports.gi.Shell.AppSystem.get_default().lookup_app('${APP}').app_info.get_filename().includes('/flatpak/exports/')")"
if [[ -n "${FLATPAK}" ]]; then
    same "6 ...which a Flatpak has" "$(jq -c '.[4]' <<<"$(menu_of "${FLATPAK}")")" true
    FID="$(ev "imports.gi.Shell.AppSystem.get_default().lookup_app('${FLATPAK}').app_info.get_string('X-Flatpak') ?? ''")"
    FID="${FID:-${FLATPAK%.desktop}}"
    user_bus systemd-run --user --collect --unit=vmlab-61-security -- amethystora-security "apps/${FID}" >/dev/null 2>&1
    FNAME="$(ev "imports.gi.Shell.AppSystem.get_default().lookup_app('${FLATPAK}').get_name()")"
    if user_bus python3 "${WORK}/atspi.py" amethystora-security wait 60 "WHAT APPS CAN REACH" >/dev/null 2>&1 &&
        user_bus python3 "${WORK}/atspi.py" amethystora-security has "${FNAME^^}" >/dev/null 2>&1; then
        ok "7 Security opens on the Apps page with that app"
    else
        not_ok "7 Security opens on the Apps page with that app" "$(user_bus python3 "${WORK}/atspi.py" amethystora-security text 2>&1 | head -20)"
    fi
    user_bus systemctl --user stop vmlab-61-security >/dev/null 2>&1
else
    skip "6-7 What can it reach? on a Flatpak (no Flatpak installed here)"
fi

# --- 8: what starts with you ---------------------------------------------------------------------------------------------
START="${HOME_DIR}/.config/autostart/vmlab-61-start.desktop"
install -d -o "${TEST_USER}" -g "${TEST_USER}" "${HOME_DIR}/.config/autostart"
printf '[Desktop Entry]\nType=Application\nName=Lab start\nExec=/usr/bin/true\n' >"${START}"
chown "${TEST_USER}:" "${START}"
ORIGINAL="$(cat "${START}")"
entry() { in_session "/usr/libexec/amethystora-startup list --json" | jq -c --arg id "$1" '.entries[] | select(.id == $id) | [.who, .state, .switch]'; }
same "8 an entry of the account's own is listed" "$(entry autostart:vmlab-61-start.desktop)" '["you","on",true]'
in_session "ame apps startup off autostart:vmlab-61-start.desktop" >/dev/null
same "8 off: exactly the Hidden shape in its place" "$(printf '[Desktop Entry]\nHidden=true\n' | cmp -s - "${START}" && echo exact)" exact
same "8 off: the entry kept aside" "$(cat "${HOME_DIR}/.local/share/amethystora/startup-off/vmlab-61-start.desktop" 2>/dev/null)" "${ORIGINAL}"
same "8 off: listed as off" "$(entry autostart:vmlab-61-start.desktop)" '["you","off",true]'
in_session "ame apps startup on autostart:vmlab-61-start.desktop" >/dev/null
same "8 on: as it was" "$(cat "${START}")" "${ORIGINAL}"
same "8 the image's own are shown and not switched" \
    "$(in_session "/usr/libexec/amethystora-startup list --json" | jq -c '[.entries[] | select(.who == "image") | .switch] | unique')" '[false]'
in_session "/usr/libexec/amethystora-startup off unit:amethystora-pkg-upgrade.timer" >/dev/null 2>&1
same "8 ...and refused when asked to" "$?" 2
same "8 Control lists them" "$(in_session "/usr/libexec/amethystora-control-list --json" | jq -c '[.items[] | select(.id == "startup") | .kind, (.rows | length > 0)]')" '["rows",true]'

# --- 9: the templates --------------------------------------------------------------------------------------------------
TEMPLATES="$(in_session "xdg-user-dir TEMPLATES")"
same "9 four templates, none executable" \
    "$(find "${TEMPLATES}" -maxdepth 1 -type f \( -name '*.txt' -o -name '*.md' -o -name '*.sh' -o -name '*.py' \) -perm 0644 | wc -l)" 4
same "9 a script with its first line" "$(head -n1 "$(find "${TEMPLATES}" -maxdepth 1 -name '*.sh' | head -n1)")" '#!/usr/bin/env bash'

# --- 10: Downloads tidied, and put back ----------------------------------------------------------------------------------
# In the home, as Downloads is: the Trash is moved into, never copied across filesystems
TIDY="${HOME_DIR}/vmlab61-downloads"
rm -rf "${TIDY}"
install -d -o "${TEST_USER}" -g "${TEST_USER}" "${TIDY}"
for name in vmlab61-setup.exe vmlab61-new.msi old-photo.jpg new-note.txt .hidden-old.txt still.part; do
    : >"${TIDY}/${name}"
done
touch -d '20 days ago' "${TIDY}/vmlab61-setup.exe"
touch -d '40 days ago' "${TIDY}/old-photo.jpg" "${TIDY}/.hidden-old.txt" "${TIDY}/still.part"
touch -d '2 days ago' "${TIDY}/vmlab61-new.msi"
chown -R "${TEST_USER}:" "${TIDY}"
MONTH="$(date -d '40 days ago' +%Y-%m)"
in_session "AMETHYSTORA_TIDY_DOWNLOADS=${TIDY} AMETHYSTORA_TIDY_NOTIFY=none /usr/libexec/amethystora-tidy-downloads run" >/dev/null
same "10 the old installer went to the Trash" "$(ls "${TIDY}/vmlab61-setup.exe" 2>/dev/null; ls "${HOME_DIR}/.local/share/Trash/files" | grep -c '^vmlab61-setup')" 1
same "10 the old photo went into its month" "$(ls "${TIDY}/${MONTH}/")" old-photo.jpg
same "10 the new ones, the hidden one and the one arriving stayed" "$(ls -A "${TIDY}" | grep -vx "${MONTH}" | sort | paste -sd ' ')" \
    ".hidden-old.txt new-note.txt still.part vmlab61-new.msi"
in_session "AMETHYSTORA_TIDY_DOWNLOADS=${TIDY} /usr/libexec/amethystora-tidy-downloads undo" >/dev/null
same "10 undo put everything back" "$(ls -A "${TIDY}" | sort | paste -sd ' ')" \
    ".hidden-old.txt new-note.txt old-photo.jpg still.part vmlab61-new.msi vmlab61-setup.exe"
rm -rf "${TIDY}"
same "10 the timer is nobody's until turned on" "$(user_bus systemctl --user is-enabled amethystora-tidy-downloads.timer 2>/dev/null)" disabled

# --- 11: the backup disk ------------------------------------------------------------------------------------------------
DISK="${WORK}/disk"
install -d -o "${TEST_USER}" -g "${TEST_USER}" "${DISK}/restic" "${HOME_DIR}/.config/amethystora"
ENV="${HOME_DIR}/.config/amethystora/backup.env"
ENV_BEFORE="$(cat "${ENV}" 2>/dev/null)"
printf 'RESTIC_REPOSITORY=%s/restic\nRESTIC_PASSWORD_FILE=/nonexistent\n' "${DISK}" >"${ENV}"
chown "${TEST_USER}:" "${ENV}"
# A mount as the volume monitor hands one over: the module asks it only for its root
ev "(() => { const m = ${UI}.get('backupdisk'); m._added({get_root: () => imports.gi.Gio.File.new_for_path('${DISK}')}); return 'added'; })()" >/dev/null
asked() { [[ "$(ev "Main.messageTray.getSources().some(s => s.notifications.some(n => n.title === 'Your backup disk'))")" == true ]]; }
if wait_for 10 asked; then ok "11 the backup disk recognized: Back up now?"; else not_ok "11 the backup disk recognized: Back up now?"; fi
ev "(() => { const m = ${UI}.get('backupdisk'); m._added({get_root: () => imports.gi.Gio.File.new_for_path('/var/tmp')}); return 'added'; })()" >/dev/null
same "11 another disk asks nothing" "$(ev "Main.messageTray.getSources().filter(s => s.notifications.some(n => n.title === 'Your backup disk')).length")" 1
ev "Main.messageTray.getSources().forEach(s => s.notifications.filter(n => n.title === 'Your backup disk').forEach(n => n.destroy())); 'gone'" >/dev/null
if [[ -n "${ENV_BEFORE}" ]]; then printf '%s\n' "${ENV_BEFORE}" >"${ENV}"; else rm -f "${ENV}"; fi

# --- 12: ten disables and enables leave GNOME's own ---------------------------------------------------------------------
in_session "ame desktop app rename ${APP} 'Lab Sixty One'" >/dev/null
in_session "ame desktop app-grid order name" >/dev/null
for _ in 1 2 3 4 5 6 7 8 9 10; do
    ev "Main.extensionManager.disableExtension('${UUID}'); Main.extensionManager.enableExtension('${UUID}'); 'again'" >/dev/null
done
wait_for 30 looks_ready
same "12 after ten restarts, still renamed" "$(ev "imports.gi.Shell.AppSystem.get_default().lookup_app('${APP}').get_name()")" "Lab Sixty One"
ev "Main.extensionManager.disableExtension('${UUID}'); 'off'" >/dev/null
sleep 2
same "12 off: its own name" "$(ev "imports.gi.Shell.AppSystem.get_default().lookup_app('${APP}').get_name()")" "${OWN}"
same "12 off: GNOME's own filter and order" \
    "$(ev "[Object.hasOwn(vm61.PCM.getDefault(), 'shouldShowApp') ? vm61.PCM.getDefault().shouldShowApp === Object.getPrototypeOf(vm61.PCM.getDefault()).shouldShowApp : true, Object.hasOwn(vm61.AD.AppDisplay.prototype, '_getItemPosition')].join(' ')")" \
    "true true"
ev "Main.extensionManager.enableExtension('${UUID}'); 'on'" >/dev/null
wait_for 30 looks_ready
same "12 the account's own layout untouched through it all" "$(gs get org.gnome.shell app-picker-layout)" "${LAYOUT_BEFORE}"

finish
