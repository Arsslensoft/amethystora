# Rendered from the current Amethystora theme. Drop a starship.toml of your own into the theme
# directory (~/.config/amethystora/themes/<theme>/) to replace it, or write ~/.config/starship.toml
# and Amethystora will leave that file alone.
#
# A powerline prompt: user@host, the folder and, in a git repository, its branch, each on a
# background of the theme's colours, then the command on a line of its own. The arrows are
# Nerd Fonts' (U+E0B0), which the image ships. Inside a repository the folder's own format
# (repo_root_format) carries the arrow that leads into the branch, and the branch's group ends the
# line with one, so that no arrow is left pointing at nothing.

format = """
$username\
$hostname\
[](fg:surface bg:accent)\
$directory\
($git_branch$git_status[](fg:repo))\
 $python\
$nodejs\
$rust\
$golang\
$container\
${env_var.AMETHYSTORA_AGENT_PROMPT}\
$cmd_duration\
$line_break\
$character"""

palette = "amethystora"

[palettes.amethystora]
surface = "{{ color0 }}"
on_surface = "{{ foreground }}"
accent = "{{ accent }}"
on_accent = "{{ background }}"
repo = "{{ color2 }}"
muted = "{{ color8 }}"
added = "{{ color2 }}"
changed = "{{ color3 }}"
removed = "{{ color1 }}"
info = "{{ color6 }}"
alt = "{{ color5 }}"

[username]
show_always = true
style_user = "fg:on_surface bg:surface bold"
style_root = "fg:removed bg:surface bold"
format = "[ $user]($style)"

[hostname]
ssh_only = false
ssh_symbol = ""
style = "fg:on_surface bg:surface bold"
format = "[@$ssh_symbol$hostname ]($style)"

[directory]
style = "fg:on_accent bg:accent bold"
read_only = " "
read_only_style = "fg:on_accent bg:accent"
truncation_length = 3
truncate_to_repo = true
format = "[  $path$read_only ]($style)[](fg:accent)"
repo_root_format = "[  $before_root_path]($before_repo_root_style)[$repo_root]($repo_root_style)[$path$read_only ]($style)[](fg:accent bg:repo)"
before_repo_root_style = "fg:on_accent bg:accent bold"
repo_root_style = "fg:on_accent bg:accent bold"

[character]
success_symbol = "[▶](bold accent)"
error_symbol = "[▶](bold removed)"
vimcmd_symbol = "[◀](bold info)"

[git_branch]
symbol = " "
style = "fg:on_accent bg:repo"
format = "[ $symbol $branch ]($style)"

[git_status]
style = "fg:on_accent bg:repo"
format = '([$all_status$ahead_behind ]($style))'

[cmd_duration]
min_time = 2000
style = "muted"
format = "[took $duration]($style) "

[python]
symbol = " "
style = "info"

[nodejs]
symbol = " "
style = "added"

[rust]
symbol = " "
style = "changed"

[golang]
symbol = " "
style = "info"

[container]
style = "muted"
format = '[$symbol \[$name\]]($style) '

# Which account of the AI agent, and whether its sandbox, an agent started here gets (ame agent hook): set only when it is
# not the home's plain one, so that work's code is never sent through the personal account unseen
[env_var.AMETHYSTORA_AGENT_PROMPT]
variable = "AMETHYSTORA_AGENT_PROMPT"
style = "fg:alt bold"
format = '[agent $env_value]($style) '
