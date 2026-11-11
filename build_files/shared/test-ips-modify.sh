#!/usr/bin/bash
# Network protection's rule changes (amethystora-ips, /usr/share/amethystora/ips): only NETWORK=block may cut anything off, so while
# watching, a rule a source ships with the action drop or reject has to alert, whichever source it came from. This applies the
# files the helper hands suricata-update (modify.conf, and modify-watch.conf unless blocking) with suricata-update's own code to
# rules of every action, and checks the helper builds that list from the two files and adds the second only when not blocking.
#
#   test-ips-modify.sh   SHARE names another copy of the folder, SURICATA_PYTHON suricata-update's modules
set -euo pipefail

SHARE="${1:-/usr/share/amethystora/ips}"
HELPER="${HELPER:-/usr/libexec/amethystora-ips}"
fail() { echo "test-ips-modify: $*" >&2; exit 1; }

# shellcheck disable=SC2016
grep -q 'cat "${SHARE}/modify-watch.conf"' "${HELPER}" || fail "the helper does not add modify-watch.conf"
grep -q '\[\[ "$(mode)" == block \]\] || cat "${SHARE}/modify-watch.conf"' "${HELPER}" || fail "modify-watch.conf is not left out while blocking"
grep -q -- '--modify-conf "${WORK}/modify.conf"' "${HELPER}" || fail "suricata-update is not given the built modify.conf"

PYTHONPATH="${SURICATA_PYTHON:-/usr/lib/suricata/python}" python3 - "${SHARE}" <<'PY'
import sys
import suricata.update.matchers as matchers
import suricata.update.rule as rules

share = sys.argv[1]


def filters(*names):
    result = []
    for name in names:
        for line in open(f'{share}/{name}'):
            line = line.strip()
            if line and not line.startswith('#'):
                result.append(matchers.ModifyRuleFilter.parse(line))
    return result


def run(chain, text):
    rule = rules.parse(text)
    assert rule is not None, text
    for one in chain:
        if one.match(rule):
            rule = one.run(rule)
    return rule.format().split(' ', 1)[0]


RULES = {
    'drop ip any any -> any any (msg:"a"; sid:1; rev:1;)': ('drop', 'alert'),
    'reject tcp any any -> any 80 (msg:"b"; sid:2; rev:1;)': ('reject', 'alert'),
    'drop tcp any any -> any 9 (msg:"c"; metadata: confidence low; sid:3; rev:1;)': ('alert', 'alert'),
    'alert ip any any -> any any (msg:"d"; content:"drop "; sid:4; rev:1;)': ('alert', 'alert'),
    'pass ip any any -> any any (msg:"e"; sid:5; rev:1;)': ('pass', 'pass'),
}
blocking = filters('modify.conf')
watching = filters('modify.conf', 'modify-watch.conf')
for text, (when_blocking, when_watching) in RULES.items():
    assert run(blocking, text) == when_blocking, f'blocking changed {text}'
    assert run(watching, text) == when_watching, f'watching left {text} as {run(watching, text)}'
PY
echo "test-ips-modify: ok"
