#!/usr/bin/bash
# What says its words in the account's language outside the apps (plans/done/PLAN-translations.md): a notification
# from a service running as root, which has no language of its own, reaches an account in the language that account
# chose (AccountsService, set as GNOME's Settings sets it) and an account with none in English; a message with a
# plural form, a button, a text that is not a message and holds `${HOME}` and `$(id)`, and the journal's line, which
# stays English; and an `ame` command's prompt in a terminal. The catalog is one this suite writes in French for the
# messages it uses (the shipped ones are still machine translations or none): it tests the mechanism, not the words.
# A stand-in notify-send logs what each account would have been shown.
# vmlab-paths: po/* @sf/usr/libexec/amethystora-notify-users @sf/usr/share/amethystora/just/* @sf/usr/lib/amethystora-*/resources/app/locale/* apps/manual/src/*
source "$(dirname "$0")/lib.sh"

# World-readable: the stand-in notify-send runs as the account
T=/var/tmp/vmlab-42-translations
rm -rf "${T}"
install -d -m 0755 "${T}" "${T}/bin" "${T}/locale/fr/LC_MESSAGES"
UID_TEST="$(id -u "${TEST_USER}")"
LOG="${T}/shown"
: >"${LOG}"
chmod 0666 "${LOG}"
HELPER=/usr/libexec/amethystora-notify-users

if ! has_session; then
    not_ok "no session for ${TEST_USER}" "$(loginctl list-sessions --no-legend)"
    finish
fi

# The account's own language is a locale of the image, as its session needs; the helper runs gettext in it
check "the French locale is installed" sh -c "locale -a | grep -qix 'fr_FR.utf-\?8'"

python3 - "${T}/locale/fr/LC_MESSAGES/amethystora.mo" <<'EOF'
import struct, sys
messages = {
    "": "Content-Type: text/plain; charset=UTF-8\nPlural-Forms: nplurals=2; plural=(n > 1);\n",
    "Security audit could not run": "L'audit de sécurité n'a pas pu s'exécuter",
    "Run it by hand with: ame security audit": "Lancez-le à la main avec : ame security audit",
    "${COUNT} security change on this machine\0${COUNT} security changes on this machine":
        "${COUNT} changement de sécurité sur cette machine\0${COUNT} changements de sécurité sur cette machine",
    "See them all with: ame security events": "Voir tout avec : ame security events",
    "Fix it…": "Corriger…",
    "A device does not work: ${device}": "Un appareil ne fonctionne pas : ${device}",
    "Toggle automatic updates?": "Activer ou couper les mises à jour automatiques ?",
    "Automatic updates are currently enabled": "Les mises à jour automatiques sont activées",
    "Automatic updates are currently disabled": "Les mises à jour automatiques sont coupées",
}
keys = sorted(messages)
ids = b""
strs = b""
offsets = []
for key in keys:
    k = key.encode()
    v = messages[key].encode()
    offsets.append((len(ids), len(k), len(strs), len(v)))
    ids += k + b"\0"
    strs += v + b"\0"
n = len(keys)
keystart = 7 * 4 + 16 * n
valuestart = keystart + len(ids)
table = b"".join(struct.pack("II", o[1], keystart + o[0]) for o in offsets)
table += b"".join(struct.pack("II", o[3], valuestart + o[2]) for o in offsets)
open(sys.argv[1], "wb").write(struct.pack("Iiiiiii", 0x950412DE, 0, n, 28, 28 + 8 * n, 0, 0) + table + ids + strs)
EOF

# A notify-send that says what it was asked to show, and to whom it was run as
cat >"${T}/bin/notify-send" <<EOF
#!/usr/bin/bash
{ printf 'as %s\n' "\$(id -nu)"; printf 'arg %s\n' "\$@"; echo end; } >>"${LOG}"
EOF
chmod 0755 "${T}/bin/notify-send"

# Run as root with the stand-in and the suite's catalog, as the services do but for these two
notify() { env PATH="${T}/bin:${PATH}" TEXTDOMAINDIR="${T}/locale" "${HELPER}" "$@"; }
shown() { cat "${LOG}" 2>/dev/null; }
set_language() { # LANGUAGE: as GNOME's Settings sets it
    busctl --system call org.freedesktop.Accounts "/org/freedesktop/Accounts/User${UID_TEST}" \
        org.freedesktop.Accounts.User SetLanguage s "$1" >/dev/null
}

set_language fr_FR.UTF-8
if wait_for 20 grep -qE '^Languages?=fr_FR.UTF-8;?$' "/var/lib/AccountsService/users/${TEST_USER}"; then
    ok "the account's language is saved where the helper reads it"
else
    not_ok "the account's language is saved where the helper reads it" "$(cat "/var/lib/AccountsService/users/${TEST_USER}" 2>&1)"
fi

# 1. A service as the watcher's scan-alert is: its messages, a French account
: >"${LOG}"
journal="$(notify --security --msgid "Security audit could not run" "Run it by hand with: ame security audit" normal)"
if grep -qx "arg L'audit de sécurité n'a pas pu s'exécuter" <(shown) && grep -q "arg Lancez-le à la main avec : ame security audit" <(shown); then
    ok "a message reaches a French account in French"
else
    not_ok "a message reaches a French account in French" "$(shown)"
fi
[[ "${journal}" == *"Security audit could not run"* && "${journal}" != *"audit de sécurité"* ]] &&
    ok "the journal's line stays English" || not_ok "the journal's line stays English" "${journal}"
grep -qx "as ${TEST_USER}" <(shown) && ok "it is shown as the account" || not_ok "it is shown as the account" "$(shown)"

# 2. The plural form, and values from the caller's environment
: >"${LOG}"
for count in 1 3; do
    COUNT="${count}" notify --security --msgid-summary --plural '${COUNT} security changes on this machine' --count "${count}" \
        --footer 'See them all with: ame security events' '${COUNT} security change on this machine' "first finding" normal >/dev/null
done
if grep -qx 'arg 1 changement de sécurité sur cette machine' <(shown) && grep -qx 'arg 3 changements de sécurité sur cette machine' <(shown) &&
    grep -q 'Voir tout avec : ame security events' <(shown); then
    ok "a plural, its number and a footer are put in the account's language"
else
    not_ok "a plural, its number and a footer are put in the account's language" "$(shown)"
fi

# 3. Text that is not a message stays as it is, whatever it holds
: >"${LOG}"
notify --security --footer 'See them all with: ame security events' 'A file named ${HOME} $(id) `id`' "" normal >/dev/null
if grep -qxF 'arg A file named ${HOME} $(id) `id`' <(shown); then
    ok "a finding's text is neither translated nor expanded"
else
    not_ok "a finding's text is neither translated nor expanded" "$(shown)"
fi

# 4. A button, and a value in a summary
: >"${LOG}"
device="Pad \${HOME}" notify --action 'fix=Fix it…' --msgid-summary 'A device does not work: ${device}' "a finding" >/dev/null
if grep -qxF 'arg --action=fix=Corriger…' <(shown) && grep -qxF 'arg Un appareil ne fonctionne pas : Pad ${HOME}' <(shown); then
    ok "a button is translated, and a value is put in once and never expanded again"
else
    not_ok "a button is translated, and a value is put in once and never expanded again" "$(shown)"
fi

# 5. An account that chose nothing gets the machine's language, English here; a language nobody ships is English
: >"${LOG}"
set_language ""
notify --security --msgid "Security audit could not run" "Run it by hand with: ame security audit" normal >/dev/null
grep -qx 'arg Security audit could not run' <(shown) && ok "an account with no language is told in English" ||
    not_ok "an account with no language is told in English" "$(shown)"
: >"${LOG}"
set_language "xx_YY.UTF-8"
notify --security --msgid "Security audit could not run" "Run it by hand with: ame security audit" normal >/dev/null
grep -qx 'arg Security audit could not run' <(shown) && ok "a language with no catalog falls back to English" ||
    not_ok "a language with no catalog falls back to English" "$(shown)"

# 6. The shipped callers still say what they said, in English, and pass the log of what they were asked
: >"${LOG}"
set_language ""
env PATH="${T}/bin:${PATH}" "/usr/libexec/amethystora-scan-alert" amethystora-lynis-audit.service >/dev/null
grep -qx 'arg Security audit could not run' <(shown) && grep -qx 'arg Run it by hand with: ame security audit' <(shown) &&
    ok "a shipped caller (the scan alert) says its message" || not_ok "a shipped caller (the scan alert) says its message" "$(shown)"
: >"${LOG}"
set_language fr_FR.UTF-8
env PATH="${T}/bin:${PATH}" TEXTDOMAINDIR="${T}/locale" "/usr/libexec/amethystora-scan-alert" amethystora-lynis-audit.service >/dev/null
grep -qx "arg L'audit de sécurité n'a pas pu s'exécuter" <(shown) &&
    ok "and in French for a French account" || not_ok "and in French for a French account" "$(shown)"

# 7. An ame command's prompt, in a terminal, in the session's language: the `ame` wrapper's text domain
out="$(in_terminal 60 'Activer ou couper' $'\x1b' -- runuser -u "${TEST_USER}" -- env LC_ALL=fr_FR.UTF-8 LANGUAGE=fr_FR:fr TEXTDOMAINDIR="${T}/locale" \
    /usr/bin/ame system auto-updates 2>&1)"
if grep -q 'Les mises à jour automatiques sont' <<<"${out}" && grep -q 'Activer ou couper les mises à jour automatiques' <<<"${out}"; then
    ok "an ame command asks in the session's language"
else
    not_ok "an ame command asks in the session's language" "${out}"
fi
out="$(in_terminal 60 'Toggle automatic updates' $'\x1b' -- runuser -u "${TEST_USER}" -- env LC_ALL=en_US.UTF-8 LANGUAGE=en \
    /usr/bin/ame system auto-updates 2>&1)"
grep -q 'Automatic updates are currently' <<<"${out}" && ok "and in English where there is no catalog" ||
    not_ok "and in English where there is no catalog" "${out}"

# 8. The shipped catalogs reach the places the helper looks in: the same caller, no catalog of this suite's
: >"${LOG}"
set_language fr_FR.UTF-8
env PATH="${T}/bin:${PATH}" "/usr/libexec/amethystora-scan-alert" amethystora-lynis-audit.service >/dev/null
grep -qx "arg L’audit de sécurité n’a pas pu s’exécuter" <(shown) &&
    ok "the shipped French catalog translates a notification" || not_ok "the shipped French catalog translates a notification" "$(shown)"
set_language ""

# 9. The Manual: a page with no translation is named in French in the sidebar, and while a translated page's headings
# do not line up with the English, a link to one of its English sections opens the English page and says so
MANUAL=/usr/share/amethystora/manual
headings() { sed '/^```/,/^```/d' "$1" | grep -cE '^#{2,6} '; }
install -m 0644 "$(dirname "$0")/atspi.py" "${T}/"
manual() { user_bus python3 "${T}/atspi.py" amethystora-manual "$@"; }
user_bus systemd-run --user --unit=vmlab-manual-fr --collect --quiet --setenv=LC_ALL=fr_FR.UTF-8 --setenv=LANGUAGE=fr \
    -- /usr/bin/amethystora-manual 'updates#rolling-back'
manual wait 30 "La liaison avec le téléphone" >/dev/null 2>&1 && ok "the sidebar names a page not translated in French" ||
    not_ok "the sidebar names a page not translated in French" "$(manual text 2>&1 | head -40)"
if [[ "$(headings "${MANUAL}/updates.md")" == "$(headings "${MANUAL}/fr/updates.md")" ]]; then
    ok "fr/updates.md lines up with the English, so its links land by themselves"
elif manual wait 10 "Afficher la page traduite" >/dev/null 2>&1 && manual click "Afficher la page traduite" >/dev/null 2>&1; then
    sleep 2
    manual has "Afficher la page traduite" >/dev/null 2>&1 && not_ok "the way back to the translated page" "$(manual text 2>&1 | head -20)" ||
        ok "a section the translation lacks opens in English, with the way back"
else
    not_ok "a section the translation lacks opens in English, with the way back" "$(manual text 2>&1 | head -20)"
fi
user_systemctl kill --kill-whom=main --signal=SIGTERM vmlab-manual-fr.service >/dev/null 2>&1
rm -rf "${T}"
finish
