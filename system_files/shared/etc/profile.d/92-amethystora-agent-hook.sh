# shellcheck shell=sh
# The shells follow the folder for the AI agent, while ame agent hook is on: /usr/share/amethystora/agent-hook/hook.sh
if [ -n "${BASH_VERSION:-}" ] && [ -r /usr/share/amethystora/agent-hook/hook.sh ]; then
    . /usr/share/amethystora/agent-hook/hook.sh
fi
