#!/usr/bin/bash
# Copying text out of a picture on the screen (plans/PLAN-screen-text.md): it is Gradia's, reached from GNOME's
# screenshot UI through Amethystora UI's screenshot module. A window shows a known sentence; Print, an area dragged over it
# and Ctrl+E (keys.py, as a person's hands), with a language file in Gradia's data folder, and the clipboard
# then holds the sentence. The language file is the image's own English pack, so that this needs no download;
# the download Gradia offers in its place is only checked to still be where it fetches it.
# vmlab-paths: @ui/*screenshot* @sf/usr/libexec/amethystora-cover-private
source "$(dirname "$0")/lib.sh"

GRADIA=be.alexandervanhee.gradia
# The screenshot module of Amethystora UI, which took Gradia's integration over (plans/PLAN-amethystora-ui.md)
EXTENSION=amethystora-ui@arsslensoft.github.io
HOME_DIR="/var/home/${TEST_USER}"
TESSDATA="${HOME_DIR}/.var/app/${GRADIA}/data/tessdata"
OWN=/run/vmlab-22
SENTENCE="Purple gems keep quiet harbors"

if ! wait_for 300 has_session; then
    skip "no session for ${TEST_USER} (10-account.sh)"
    finish
fi
# The default Flatpaks install on the first boots with a network, retried until there is one (25-every-app.sh):
# Gradia is waited for itself, and with no network at all there is nothing to check
gradia_installed() { user_bus flatpak info "${GRADIA}" >/dev/null 2>&1; }
if ! wait_for 600 gradia_installed; then
    if ! getent hosts dl.flathub.org >/dev/null; then
        skip "the machine has no network, so Gradia was not installed from Flathub"
        finish
    fi
    if ((SUITE_PHASE < 4)); then
        echo "# Gradia was still not installed after 10 minutes: on with it after a restart"
        reboot_and_continue
    fi
    not_ok "Gradia was not installed" "$(systemctl status flatpak-preinstall.service --no-pager 2>&1 | tail -n 8)"
    finish
fi

keys() { python3 "$(dirname "$0")/keys.py" "$@"; }
shell() { user_bus python3 "${OWN}/atspi.py" gnome-shell "$@"; }
mkdir -p "${OWN}"
install -m 0644 "$(dirname "$0")/atspi.py" "$(dirname "$0")/screentext.py" "${OWN}/"
chmod -R a+rX "${OWN}"

ext_state() { user_bus gnome-extensions info "${EXTENSION}" 2>/dev/null | sed -n 's/^ *State: *//p'; }
ext_active() { [[ "$(ext_state)" == ACTIVE ]]; }
# The first session of a disk made a minute ago, with Flatpaks installing under it, may still be loading its
# extensions in software rendering; the second boot of it is settled
if ((SUITE_PHASE < 2)) && ! wait_for 120 ext_active; then
    echo "# the shell had not enabled Amethystora UI after 2 minutes: on with it after a restart"
    reboot_and_continue
fi
if wait_for 60 ext_active; then
    ok "Amethystora UI is active"
else
    not_ok "Amethystora UI is not active" "$(user_bus gnome-extensions info "${EXTENSION}" 2>&1)" \
        "enabled: $(user_bus gsettings get org.gnome.shell enabled-extensions 2>&1 | cut -c1-600)" \
        "disabled: $(user_bus gsettings get org.gnome.shell disabled-extensions 2>&1)" \
        "disable-user-extensions: $(user_bus gsettings get org.gnome.shell disable-user-extensions 2>&1)" \
        "all: $(for e in $(user_bus gnome-extensions list 2>/dev/null); do echo "${e%%@*}=$(user_bus gnome-extensions info "${e}" 2>/dev/null | sed -n 's/^ *State: *//p')"; done | paste -sd ' ')" \
        "shell: $(user_bus journalctl --user -b --no-pager -o cat -t gnome-shell 2>/dev/null | grep -iv 'vulkan\|mesa' | grep -i 'extension\|gradia\|JS ERROR\|screenshot' | tail -n 12)" \
        "$(user_bus journalctl --user -b --no-pager -o cat 2>/dev/null | grep -i 'gradia' | grep -v Vulkan | tail -n 8)"
    # The rest of the flow is still tried, with the extension turned off and on as at a new login
    user_bus gnome-extensions disable "${EXTENSION}"
    sleep 2
    user_bus gnome-extensions enable "${EXTENSION}"
    wait_for 30 ext_active && echo "# enabled again by hand: ACTIVE" || echo "# enabled again by hand: $(ext_state)"
fi

# Where Gradia's own download would come from
code=000
for _ in 1 2 3; do
    code="$(curl -sIL -o /dev/null -w '%{http_code}' --max-time 30 \
        https://github.com/tesseract-ocr/tessdata_best/raw/4.1.0/eng.traineddata)" && [[ "${code}" == 200 ]] && break
    sleep 5
done
case "${code}" in
    200) ok "the language data Gradia downloads is where it fetches it" ;;
    000) skip "no network to GitHub: the download Gradia offers was not checked" ;;
    *) not_ok "Gradia's language download answers ${code}" ;;
esac

# The language chosen, as the download would leave it
install -d -o "${TEST_USER}" -g "${TEST_USER}" "${HOME_DIR}/.var" "${HOME_DIR}/.var/app" "${HOME_DIR}/.var/app/${GRADIA}" \
    "${HOME_DIR}/.var/app/${GRADIA}/data" "${TESSDATA}"
install -o "${TEST_USER}" -g "${TEST_USER}" -m 0644 /usr/share/tesseract/tessdata/eng.traineddata "${TESSDATA}/"
chcon -R --reference="${HOME_DIR}/.var" "${HOME_DIR}/.var/app" 2>/dev/null

user_bus systemd-run --user --quiet --collect --unit=vmlab-screentext --setenv=WAYLAND_DISPLAY=wayland-0 -- \
    python3 "${OWN}/screentext.py" "${SENTENCE}"
sleep 8
# Something else on the clipboard first, so that what is there afterwards is what the button copied
user_bus systemd-run --user --quiet --collect --unit=vmlab-before -- wl-copy --foreground "vmlab-before-$RANDOM"
sleep 2

# Print opens the screenshot UI; the area over the sentence is dragged, as a person does; Extract Text is Ctrl+E
keys print
sleep 4
keys drag:0.08,0.3,0.92,0.7
sleep 2
keys ctrl+e
# Gradia reads the picture, shows the text in its Text Extraction window and copies it itself (its standalone
# dialog sets auto_copy; a client may set the clipboard on Wayland only while it has the keyboard, so nothing else is
# allowed to take it meanwhile). What is checked is what a person sees there, through the accessibility bus: the
# clipboard cannot be read back by a test in this lab (wl-paste says "Nothing is copied" to a session with no window of
# its own, and the history menu holds nothing without a keyring).
sleep 20
recognised() { user_bus python3 "${OWN}/atspi.py" gradia wait 10 "harbors" >/dev/null 2>&1; }
if wait_for 150 recognised; then
    ok "Print, an area and Ctrl+E have Gradia read the sentence in its Text Extraction window"
else
    not_ok "Gradia's Text Extraction window does not show the sentence" \
        "Gradia: $(user_bus flatpak ps --columns=application,instance 2>&1 | head -n 5)" \
        "Gradia: $(user_bus python3 "${OWN}/atspi.py" gradia text 2>&1 | head -n 20)" \
        "screen: $(shell text 2>&1 | head -n 30)" \
        "$(user_bus journalctl --user -b --since '-5min' --no-pager -o cat 2>/dev/null | grep -i 'gradia\|ocr\|screenshot' | grep -v Vulkan | tail -n 15)" \
        "journal: $(user_bus journalctl --user -b --since '-6min' --no-pager -o cat 2>/dev/null | grep -v 'Vulkan\|libEGL\|MESA\|usb' | tail -n 45)"
fi
# The capture is kept like any screenshot
found="$(find "${HOME_DIR}/Pictures" -type f -newer "${OWN}/screentext.py" 2>/dev/null)"
if [[ -n "${found}" ]]; then
    ok "the capture is in the screenshot folder, as the manual says"
else
    not_ok "no capture was kept" "$(find "${HOME_DIR}/Pictures" 2>&1 | head -n 10)"
fi

user_bus flatpak kill "${GRADIA}" >/dev/null 2>&1
user_systemctl stop vmlab-screentext.service vmlab-before.service >/dev/null 2>&1
[[ -z "${found}" ]] || rm -f ${found}
finish
