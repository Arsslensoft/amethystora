#!/usr/bin/bash
# Looking for updates without installing them (amethystora-update-check, `ame update check`, System Updates' Waiting
# to be installed), in the test account's session:
# - the daily timer enabled, the notice unit beside it, and uupd waiting for a look rather than failing beside it;
# - `ame update check` as the account starts the look without a password, and what it wrote is readable by everyone and
#   names the deployment running now; where the lab's image is in no registry the look fails, and then says why;
# - the driver `ame update` and uupd pick: bootc on this machine, which has nothing layered;
# - System Updates, as a person reads it, from a finding planted for the deployment running now: a new system and a
#   layered package, under Waiting to be installed, the top saying an update is waiting; and nothing of it once the
#   finding is from another deployment. What was found is put back afterwards.
# The notice while automatic updates are off is held to its words by test-update-check.sh, at build.
# vmlab-paths: @sf/usr/libexec/amethystora-update-check @sf/usr/lib/systemd/system/amethystora-update-check* @sf/usr/lib/systemd/system/uupd*.service.d/* @sf/*just/update.just @sf/usr/share/polkit-1/rules.d/40-amethystora-update-check.rules apps/update/*
source "$(dirname "$0")/lib.sh"

HERE="$(dirname "$0")"
HELPER=/usr/libexec/amethystora-update-check
FOUND=/var/lib/amethystora/updates/available.json
# atspi.py where the account can run it: the suites' own folder is root's
ATSPI=/run/vmlab-update-check/atspi.py
same() { if [[ "$2" == "$3" ]]; then ok "$1"; else not_ok "$1" "expected: $3" "found: $2"; fi; }
atspi() { user_bus python3 "${ATSPI}" amethystora-update "$@"; }

wait_for 300 has_session || not_ok "the test account has a session"

# --- The units --------------------------------------------------------------------------------------------------

same "the daily look is on as the image ships it" "$(systemctl is-enabled amethystora-update-check.timer 2>&1)" enabled
check "the notice has its unit" systemctl cat amethystora-update-check-notice.service
for unit in uupd.service uupd-manual.service; do
    systemctl show -p After "${unit}" | grep -q amethystora-update-check.service &&
        ok "${unit} waits for a look under way" || not_ok "${unit} waits for a look under way"
done

# --- A look, as the account -----------------------------------------------------------------------------------------

BOOTED="$(rpm-ostree status --json | jq -r '[.deployments[] | select(.booted)][0].checksum')"
SAVED=""
[[ -e "${FOUND}" ]] && SAVED="$(cat "${FOUND}")"
# From the account's own manager, as the session's programs run: polkit takes that for the person at the machine
OUT="$(in_session timeout 600 ame update check 2>&1)"
if [[ -s "${FOUND}" ]]; then
    ok "ame update check looked, without a password"
    same "what was found is readable by everyone" "$(stat -c %a "${FOUND}")" 644
    same "and names the system running now" "$(jq -r .booted "${FOUND}")" "${BOOTED}"
    ERROR="$(jq -r '.error // ""' "${FOUND}")"
    if [[ -z "${ERROR}" ]]; then
        jq -e '.driver == "bootc" and (.packages | type) == "array" and has("system")' "${FOUND}" >/dev/null &&
            ok "the look found what is waiting: $(jq -c '{system, packages}' "${FOUND}")" || not_ok "the look's record" "$(cat "${FOUND}")"
    else
        grep -qF "${ERROR}" <<<"${OUT}" && ok "a look that could not reach the image says why: ${ERROR}" ||
            not_ok "a failed look says why" "${OUT}"
    fi
else
    not_ok "ame update check looked, without a password" "${OUT}"
fi
same "this machine, with nothing layered, is updated with bootc" "$("${HELPER}" driver)" bootc

# --- System Updates, from a finding of the system running now ---------------------------------------------------------

jq -nc --arg booted "${BOOTED}" --argjson now "$(date +%s)" '{checked: $now, booted: $booted, driver: "rpm-ostree",
    system: {version: "44.29991231", built: $now, digest: "sha256:vmlab"},
    packages: [{name: "vmlab-layered", from: "1.0-1.fc44", to: "1.1-1.fc44"}], error: null}' >"${FOUND}"
install -d -m 0755 "$(dirname "${ATSPI}")"
install -m 0644 "${HERE}/atspi.py" "${ATSPI}"
open_updates() {
    user_bus systemctl --user stop vmlab-update-check-window.service 2>/dev/null || true
    user_bus systemd-run --user --unit=vmlab-update-check-window --collect --quiet --setenv=LC_ALL=en_US.UTF-8 \
        --setenv=LANGUAGE=en -- /usr/bin/amethystora-update
}
open_updates
if atspi wait 120 "Waiting to be installed" >/dev/null 2>&1; then
    ok "System Updates has a section for what is waiting"
    atspi has "Amethystora 44.29991231" >/dev/null 2>&1 && ok "it names the new system" || not_ok "it names the new system" "$(atspi text 2>&1 | head -n 40)"
    atspi has "vmlab-layered" >/dev/null 2>&1 && atspi has "From 1.0-1.fc44 to 1.1-1.fc44" >/dev/null 2>&1 &&
        ok "and the layered package, from which version to which" || not_ok "the layered package" "$(atspi text 2>&1 | head -n 40)"
    atspi has "An update is waiting" >/dev/null 2>&1 && ok "the top says an update is waiting" || not_ok "the top says an update is waiting" "$(atspi text 2>&1 | head -n 20)"
else
    not_ok "System Updates has a section for what is waiting" "$(atspi text 2>&1 | head -n 40)"
fi
# From another deployment it says nothing of this one
jq -c '.booted = "another"' "${FOUND}" >"${FOUND}.new" && mv "${FOUND}.new" "${FOUND}" && chmod 0644 "${FOUND}"
open_updates
if atspi wait 120 "Not looked since this version started" >/dev/null 2>&1; then
    atspi has "vmlab-layered" >/dev/null 2>&1 && not_ok "a finding from another deployment is shown" || ok "a finding from another deployment is not shown"
else
    not_ok "a finding from another deployment is left out" "$(atspi text 2>&1 | head -n 40)"
fi
user_bus systemctl --user stop vmlab-update-check-window.service 2>/dev/null || true

# Put back as it was
if [[ -n "${SAVED}" ]]; then printf '%s\n' "${SAVED}" >"${FOUND}"; else rm -f "${FOUND}"; fi
rm -rf "$(dirname "${ATSPI}")"
finish
