# shellcheck shell=bash

# A command this computer lacks: where it can come from, instead of a bare "command not found", while
# `ame desktop missing-command` is on (read when the shell starts). It only suggests: nothing is installed.
# zsh's is in /etc/zsh/zshrc, fish's in /usr/share/fish/vendor_conf.d/amethystora-missing-command.fish.
if [ -n "${BASH_VERSION-}" ] && [ -e "${XDG_CONFIG_HOME:-$HOME/.config}/amethystora/missing-command" ]; then
	case "$-" in
	*i*)
		command_not_found_handle() {
			if [ -x /usr/libexec/amethystora-missing-command ] && /usr/libexec/amethystora-missing-command "$1" >&2; then
				return 127
			fi
			printf 'bash: %s: command not found\n' "$1" >&2
			return 127
		}
		;;
	esac
fi
