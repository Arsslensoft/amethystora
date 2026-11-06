#!/usr/bin/bash
# A GitHub repository's latest workflow run, in the pill: its state, and a button that opens the run.
# Its settings come on a pipe (descriptor 10), never in the environment: `ame plugin set ID NAME` gives one (a secret goes into the Vault).
# What it prints is one line of JSON, built by jq so that any text in it is escaped. The network is only the hosts its manifest names.
set -euo pipefail
settings='{}'
read -r -t 1 -u 10 settings 2>/dev/null || true
setting() { jq -r --arg k "$1" '.[$k] // empty' <<<"${settings}"; }

repo="$(setting repo)"
token="$(setting token)"
if [[ ! "${repo}" =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]]; then
    jq -nc '{text: "Needs a repository", icon: "dialog-question-symbolic", card: [{type: "text", text: "Give it one with: ame plugin set <id> repo"}]}'
    exit 0
fi
auth=()
# A token goes to curl in a config it reads from a descriptor, never on the command line
[[ -z "${token}" ]] || auth=(--config <(printf 'header = "Authorization: Bearer %s"\n' "${token}"))
run="$(curl -fsS --max-time 15 "${auth[@]}" -H 'Accept: application/vnd.github+json' "https://api.github.com/repos/${repo}/actions/runs?per_page=1" | jq -c '.workflow_runs[0] // empty')" || run=""
if [[ -z "${run}" ]]; then
    jq -nc '{hidden: true, text: "No runs", icon: "emblem-synchronizing-symbolic"}'
    exit 0
fi
if [[ "${PLUGIN_ACTION:-}" == open-run ]]; then
    plugin open "$(jq -r .html_url <<<"${run}")" >/dev/null || true
fi
jq -c '
    (.status) as $s | (.conclusion // "") as $c |
    {
        text: "\(.name) · \(if $s == "completed" then $c else $s end)",
        icon: (if $c == "success" then "emblem-ok-symbolic" elif $c == "failure" then "dialog-error-symbolic" else "emblem-synchronizing-symbolic" end),
        class: (if $c == "success" then "good" elif $c == "failure" then "critical" else "accent" end),
        hidden: ($s == "completed" and $c == "success"),
        card: [{type: "row", label: "Branch", value: .head_branch}, {type: "row", label: "Run", value: "#\(.run_number)"},
               {type: "button", label: "Open the run", action: "open-run"}]
    }' <<<"${run}"
