#!/usr/bin/bash
# The logo menu (plans/done/PLAN-logo-menu.md): Amethystora's own extension at the left end of the top bar, used as a
# person would, through GNOME Shell's accessibility tree and a pointer of the lab's own (keys.py). Two phases:
#   1. On, and nothing of the Logo Menu it replaced; leftmost in the top bar; its entries in order, each opening
#      its app; a middle click showing and hiding the overview and opening no menu; Mission Center taken away
#      hiding its entry and nothing installing it; back and leftmost after the screen lock. Then the account
#      given the list of extensions an account kept from before the swap, for the next login.
#   2. After a restart and that login: the new extension turned on by the image's own hook, and leftmost.
# vmlab-paths: @sf/usr/share/gnome-shell/extensions/amethystora-logo-menu@*
source "$(dirname "$0")/lib.sh"

UUID=amethystora-logo-menu@arsslensoft.github.io
OWN=/run/vmlab-26
HOME_DIR="/var/home/${TEST_USER}"
ENTRIES='About this computer|Manual|-|Settings|Software centre|-|Extensions|System monitor|Terminal|-|Containers'

if ! wait_for 300 has_session; then
    skip "no session for ${TEST_USER} (10-account.sh)"
    finish
fi

mkdir -p "${OWN}"
install -m 0644 "$(dirname "$0")/atspi.py" "${OWN}/"
chmod -R a+rX "${OWN}"
keys() { python3 "$(dirname "$0")/keys.py" "$@"; }
shell() { user_bus python3 "${OWN}/atspi.py" gnome-shell "$@"; }
# What GNOME Shell shows now, from its accessibility tree (the items of a closed menu are still "visible" to
# atspi.py, so these go by what is on the screen): `panel`, the left edge of the logo menu ("logo X") and of
# everything else in the top bar ("other X"); `menu`, the menu items shown, a line between two groups as -;
# `at NAME ROLE`, the middle of the one shown, as fractions of the screen for keys.py; `windows`, the apps with a
# window
tree() {
    user_bus python3 - "$@" <<'EOF'
import sys
sys.path.insert(0, "/run/vmlab-26")
from atspi import Atspi, application, walk, words

shell = application("gnome-shell")


def box(node):
    return Atspi.Component.get_extents(node, Atspi.CoordType.SCREEN)


def shown(node):
    try:
        return node.get_state_set().contains(Atspi.StateType.SHOWING)
    except Exception:
        return False


# A menu item's own name is empty: what it says is its label's
def name(node):
    if node.get_role_name() == "menu item":
        return next((w for i in range(node.get_child_count()) for w in words(node.get_child_at_index(i))), "")
    return node.get_name() or ""


if sys.argv[1] == "panel":
    logo = next((n for n, _ in walk(shell) if n.get_name() == "Logo menu" and shown(n)), None)
    if logo is None:
        sys.exit("no Logo menu button shown")
    top = box(logo)
    print(f"logo {top.x}")
    for node, _ in walk(shell):
        if node is logo or not shown(node) or node.get_role_name() not in ("menu", "push button", "toggle button", "label"):
            continue
        b = box(node)
        # In the top bar, and not inside the logo menu's own button
        if b.width > 0 and b.y + b.height <= top.y + top.height + 4 and not top.x <= b.x < top.x + top.width:
            print(f"other {b.x} {node.get_role_name()} {words(node)[:1]}")
elif sys.argv[1] == "menu":
    for node, _ in walk(shell):
        if node.get_role_name() in ("menu item", "separator") and shown(node):
            print(name(node) if node.get_role_name() == "menu item" else "-")
elif sys.argv[1] == "at":
    node = next((n for n, _ in walk(shell) if name(n) == sys.argv[2] and n.get_role_name() == sys.argv[3] and shown(n)), None)
    if node is None:
        sys.exit(f"nothing shown named {sys.argv[2]!r}")
    stage = shell.get_child_at_index(0)
    screen = max((box(stage.get_child_at_index(i)) for i in range(stage.get_child_count())),
                 key=lambda b: b.width * b.height if b.width > 0 and b.height > 0 else 0)
    b = box(node)
    print(f"{(b.x + b.width / 2 - screen.x) / screen.width:.4f},{(b.y + b.height / 2 - screen.y) / screen.height:.4f}")
elif sys.argv[1] == "windows":
    desktop = Atspi.get_desktop(0)
    for i in range(desktop.get_child_count()):
        app = desktop.get_child_at_index(i)
        if app is not None and app.get_name() != "gnome-shell" and app.get_child_count() > 0:
            print(app.get_name())
EOF
}
# The session locks itself after five idle minutes, and on a new machine this suite first waits longer than that
# for the default Flatpaks: nothing in the top bar is shown then. Unlocked before each look at it, as a person would
wake() {
    screen_locked || return 0
    loginctl unlock-session "$(session_id)"
    wait_for 30 screen_unlocked
    sleep 3
}
menu_items() { tree menu | paste -sd '|'; }
menu_open() { [[ "$(menu_items)" == *Manual* ]]; }
# The windows AT-SPI lists, and kitty's: kitty draws with OpenGL, not GTK, so nothing of its windows is on the accessibility
# bus. It exits at once when it cannot open a window, so a kitty that is running has one (the 10 seconds below hold it to that)
windows() { { tree windows; pgrep -u "${TEST_USER}" -x kitty | sed 's/^/kitty-/'; } | sort -u; }
leftmost() {
    local out logo
    wake
    out="$(tree panel 2>&1)" || { echo "${out}"; return 1; }
    logo="$(awk '$1 == "logo" { print $2 }' <<<"${out}")"
    awk -v logo="${logo}" '$1 == "other" && $2 < logo { bad = 1; print } END { exit bad }' <<<"${out}"
}
ext_state() { user_bus gnome-extensions info "$1" 2>/dev/null | sed -n 's/^ *State: *//p'; }
ext_active() { [[ "$(ext_state "${UUID}")" == ACTIVE ]]; }
overview() {
    user_bus gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell \
        --method org.freedesktop.DBus.Properties.Get org.gnome.Shell OverviewActive 2>/dev/null
}
overview_is() { [[ "$(overview)" == "(<$1>,)" ]]; }
session_id() { loginctl show-user "${TEST_USER}" -p Display --value; }
screen_locked() { [[ "$(loginctl show-session "$(session_id)" -p LockedHint --value 2>/dev/null)" == yes ]]; }
screen_unlocked() { ! screen_locked; }
# The menu opened with a click on the button, where it is. A click made while an app's first start keeps the
# VM's processors busy (Bazaar's took 25 s of CPU) can be lost, as on a slow machine: a person clicks again
open_menu() {
    local at
    wake
    at="$(tree at "Logo menu" menu)" || return 1
    keys "click:${at}"
    wait_for 10 menu_open && return
    echo "# a click on the logo opened nothing in 10 s: clicked again"
    keys "click:${at}"
    wait_for 10 menu_open
}
close_menu() { ! menu_open || keys esc; }
# What an entry of the loop below starts, a Flatpak by its ID or a program by its command line, and whether a
# window has opened that was not there before
running() {
    if [[ "${flatpak}" != - ]]; then
        user_bus flatpak ps --columns=application 2>/dev/null | grep -qxF -- "${flatpak}"
    else
        pgrep -u "${TEST_USER}" -f -- "${process}" >/dev/null
    fi
}
stopped() { ! running; }
stop() {
    if [[ "${flatpak}" != - ]]; then
        user_bus flatpak kill "${flatpak}" >/dev/null 2>&1
    else
        pkill -TERM -u "${TEST_USER}" -f -- "${process}"
    fi
    wait_for 15 stopped || true
}
new_window() { [[ -n "$(comm -13 <(echo "${before}") <(windows))" ]]; }

# A suite before this one may have left the screen locked, which turns every extension off, or every extension off
screen_locked && loginctl unlock-session "$(session_id)" && wait_for 30 screen_unlocked
if [[ "$(user_bus gsettings get org.gnome.shell disable-user-extensions 2>/dev/null)" == true ]]; then
    echo "# a suite before this one left every extension off (GNOME Shell crashed): turned back on"
    user_bus gsettings set org.gnome.shell disable-user-extensions false
fi
if shell has "Welcome to" >/dev/null; then
    keys esc
    sleep 2
fi
# GNOME's overview, where a session starts: a click on its workspace leaves it
overview_is true && keys click:0.5,0.5

case "${SUITE_PHASE}" in
1)
    # 1. On, and the Logo Menu gone with its helpers
    if wait_for 60 ext_active; then
        ok "the logo menu is on in the session"
    else
        not_ok "the logo menu is not on" "$(user_bus gnome-extensions info "${UUID}" 2>&1)"
    fi
    check "the Logo Menu is not installed" test ! -e /usr/share/gnome-shell/extensions/logomenu@aryan_k
    check "no helper that installed Mission Center from a click" test ! -e /usr/bin/missioncenter-helper
    check "no helper that installed DistroShelf from a click" test ! -e /usr/bin/distroshelf-helper
    said="$(user_bus journalctl --user -b --no-pager -o cat 2>/dev/null | grep -F "${UUID}" | tail -n 5)"
    [[ -z "${said}" ]] && ok "GNOME Shell logged nothing about it" || not_ok "GNOME Shell logged about it" "${said}"

    # The default Flatpaks, which three entries open, install on the first boots with a network (25-every-app.sh)
    preinstalled() { [[ "$(systemctl show flatpak-preinstall.service -p SubState -p Result --value | paste -sd ' ')" == "exited success" ]]; }
    if ! wait_for 600 preinstalled; then
        not_ok "flatpak-preinstall.service did not finish" "$(systemctl status flatpak-preinstall.service --no-pager 2>&1 | tail -n 8)"
        finish
    fi

    # 2. The leftmost thing in the top bar, left of the workspace row
    check "the logo menu is the leftmost item in the top bar" leftmost
    echo "# panel: $(tree panel 2>&1 | head -n 6 | paste -sd ';')"

    # 3. The eight entries in their four groups, each opening its app
    if open_menu; then
        sleep 2
        items="$(menu_items)"
        [[ "${items}" == "${ENTRIES}" ]] && ok "the menu shows its eight entries in order" ||
            not_ok "the menu's entries are not the eight in order" "shows: ${items}" "wants: ${ENTRIES}"
        close_menu
    else
        not_ok "a click on the logo did not open its menu" "$(tree panel 2>&1 | head -n 5)"
    fi
    sleep 2
    while IFS='|' read -r label flatpak process; do
        # Nothing of it running yet, so that what opens is the entry's
        stop
        sleep 2
        before="$(windows)"
        if ! open_menu; then
            not_ok "${label}: the menu did not open"
            continue
        fi
        if ! at="$(tree at "${label}" "menu item")"; then
            not_ok "${label}: no such entry in the menu"
            close_menu
            continue
        fi
        keys "click:${at}"
        # Open, and still open 10 seconds later, as 25-every-app.sh holds the app grid to: an app closed while it
        # is still starting (Bazaar's first start takes 25 s of CPU here) once left GNOME taking no more clicks
        if wait_for 60 running && wait_for 30 new_window && sleep 10 && running; then
            ok "${label} opens its app's window"
        elif running; then
            not_ok "${label} started its app, but no window of it showed" \
                "windows before: $(paste -sd , <<<"${before}"), now: $(windows | paste -sd ,)"
        else
            not_ok "${label} did not start its app" "$(user_bus journalctl --user -b --since -90s --no-pager -o cat 2>/dev/null | tail -n 10)"
        fi
        menu_open && not_ok "${label}: the menu stayed open after its entry was chosen"
        if [[ "${label}" == "About this computer" ]]; then
            check "About this computer opens Settings on its About page" pgrep -u "${TEST_USER}" -f "gnome-control-center system about"
        fi
        stop
        sleep 2
    done <<'EOF'
About this computer|-|gnome-control-center
Manual|-|amethystora-manual
Settings|-|gnome-control-center
Software centre|io.github.kolunmi.Bazaar|
Extensions|-|gnome-extensions-app
System monitor|io.missioncenter.MissionCenter|
Terminal|-|kitty
Containers|com.ranfdev.DistroShelf|
EOF
    # DistroShelf opens a container's terminal in the one it was set to, or else the one it finds: kitty or Ptyxis here. The
    # Logo Menu's helper set it only when it installed DistroShelf itself, so the setting was never the image's.
    terminal="$(user_bus flatpak run --command=gsettings com.ranfdev.DistroShelf get com.ranfdev.DistroShelf selected-terminal 2>&1)"
    echo "# DistroShelf's selected-terminal: ${terminal}"
    [[ "${terminal}" == *kitty* || "${terminal}" == *tyxis* || "${terminal}" == "''" ]] && ok "DistroShelf's terminal is kitty, Ptyxis, or the one it finds" ||
        not_ok "DistroShelf's terminal is neither kitty nor Ptyxis" "${terminal}"

    # 4. A middle click shows the overview and opens no menu; a second one hides it
    wake
    if at="$(tree at "Logo menu" menu)"; then
        keys "middle:${at}"
        if wait_for 10 overview_is true; then
            ok "a middle click shows the overview"
        else
            not_ok "a middle click did not show the overview" "$(overview)"
        fi
        if menu_open; then
            not_ok "a middle click opened the menu too"
            keys esc
        fi
        keys "middle:${at}"
        if wait_for 10 overview_is false; then
            ok "a second middle click hides the overview"
        else
            not_ok "a second middle click did not hide the overview" "$(overview)"
            keys esc
        fi
    else
        not_ok "no logo menu to click" "${at}"
    fi

    # 5. An app taken away hides its entry, and nothing installs it
    if curl -sfI --max-time 15 https://dl.flathub.org/repo/summary.idx >/dev/null; then
        flatpak uninstall --system -y --noninteractive io.missioncenter.MissionCenter >/dev/null 2>&1
        sleep 5
        if open_menu; then
            sleep 2
            items="$(menu_items)"
            [[ "${items}" == "${ENTRIES/|System monitor/}" ]] && ok "with Mission Center gone, its entry is hidden" ||
                not_ok "with Mission Center gone, the menu shows otherwise" "shows: ${items}"
            close_menu
        else
            not_ok "the menu did not open without Mission Center"
        fi
        sleep 5
        if flatpak info io.missioncenter.MissionCenter >/dev/null 2>&1; then
            not_ok "Mission Center came back without being asked for"
        else
            ok "nothing installed Mission Center"
        fi
        check "Mission Center installs again from Flathub" flatpak install --system -y --noninteractive flathub io.missioncenter.MissionCenter
    else
        skip "Mission Center taken away: no network to install it again"
    fi

    # 6. The screen lock turns every extension off; unlocking brings the button back, still leftmost
    loginctl lock-session "$(session_id)"
    if wait_for 30 screen_locked; then
        sleep 3
        keys esc
        sleep 2
        keys "text:${TEST_USER}" return
        if wait_for 30 screen_unlocked; then
            overview_is true && keys click:0.5,0.5
            wait_for 30 ext_active && ok "after the screen lock the logo menu is on again" ||
                not_ok "after the screen lock the logo menu is off"
            check "after the screen lock the logo menu is still leftmost" leftmost
        else
            not_ok "the screen did not unlock with the password"
            loginctl unlock-session "$(session_id)"
        fi
    else
        not_ok "the screen did not lock"
    fi

    # 7. An account that kept its list of extensions from before the swap: the Logo Menu named and this one not, and
    # the image's hook not yet run for this image. Its next login is the next phase's.
    defaults="$(GSETTINGS_BACKEND=memory gsettings get org.gnome.shell enabled-extensions)"
    old="$(sed -e "s/, '${UUID}'//" -e "s/^\[/['logomenu@aryan_k', /" <<<"${defaults}")"
    user_bus gsettings set org.gnome.shell enabled-extensions "${old}"
    rm -f "${HOME_DIR}/.local/state/amethystora/extensions-image"
    reboot_and_continue
    ;;
2)
    list="$(user_bus gsettings get org.gnome.shell enabled-extensions)"
    [[ "${list}" == *"'${UUID}'"* ]] && ok "an account from before the swap has the logo menu turned on at its next login" ||
        not_ok "an account from before the swap does not have the logo menu turned on" "${list}"
    [[ "$(ext_state logomenu@aryan_k)" != ACTIVE ]] && ok "the Logo Menu it still names is nothing" ||
        not_ok "the Logo Menu is active"
    wait_for 60 ext_active && ok "the logo menu is on after that login" || not_ok "the logo menu is off after that login"
    check "the logo menu is leftmost for that account" leftmost
    echo "# panel: $(tree panel 2>&1 | head -n 6 | paste -sd ';')"
    # The run's screenshot after this suite shows the top bar's left end, to compare with a build before the swap
    overview_is true && keys click:0.5,0.5
    ;;
esac
finish
