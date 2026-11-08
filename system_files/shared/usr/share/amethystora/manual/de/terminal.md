# Das Terminal

Amethystora ist für Menschen gemacht, die die Befehlszeile mögen, genauso wie für Menschen, die sie nie
öffnen. Wenn Sie es doch tun, ist sie für Sie bereit.

## Eines öffnen

`Super+Return` öffnet ein Terminal, ebenso `Ctrl+Alt+T` und `Ctrl+Alt+Return`. Das Terminal ist
Ptyxis. Es kennt Container: Das Menü neben dem Knopf für einen neuen Reiter öffnet einen Reiter auf dem
Host oder in einem Ihrer [Distrobox- und Toolbox-Container](containers.md).

Seine Farben folgen dem [Thema](themes.md), ebenso der Prompt.

## Die Begrüßung

Ein neues Terminal grüßt mit dem Amethystora-Logo, einer Übersicht über den Rechner, dem Abbild, das Sie
verwenden, ein paar nützlichen Befehlen und einem Tipp. `ame desktop greeting` schaltet sie aus und wieder
ein.

`fetch` zeigt das Logo und die Systemübersicht jederzeit erneut an. Ein Glanz huscht über die Facetten
des Logos, bevor die Übersicht erscheint; stattdessen bekommen Sie das unbewegte Logo über SSH, in der
Textkonsole, in einem Terminal ohne volle Farbunterstützung, wenn `NO_COLOR` gesetzt ist, oder wenn
**Animationen reduzieren** in **Einstellungen → Barrierefreiheit** eingeschaltet ist. Alles, was Sie nach
`fetch` angeben, geht an fastfetch, das es ausführt.

## Shells

Die Shell ist bash, mit dem Starship-Prompt. fish und zsh sind ebenfalls installiert. Gewechselt wird im
Terminal statt systemweit, damit eine kaputte Shell-Konfiguration Sie nie von der Anmeldung aussperren
kann:

1. Öffnen Sie die **Einstellungen** des Terminals und bearbeiten Sie Ihr Profil.
2. Schalten Sie **Benutzerdefinierten Befehl verwenden** ein und geben Sie `/usr/bin/fish` oder
   `/usr/bin/zsh` ein.

## ame

`ame` führt die Befehle aus, die mit dem System kommen: kleine, getestete Skripte für die Aufgaben, die
sonst eine Websuche bräuchten, in Gruppen wie `desktop`, `security` und `system`. Dieses Handbuch erwähnt
sie dort, wo sie helfen; um alle zu sehen:

```bash
ame                           # the groups, and the commands outside them
ame security                  # the commands in one group, with a line about what each does
ame --list --list-submodules  # every command, in every group
ame -n security status        # print what a command would run, without running it
```

`ame pkg` ist [amethystora-pkg](containers.md#packages-from-other-distributions). `ujust`, der ältere Name
von `ame`, führt dieselben Befehle aus, und ihre Namen aus der Zeit vor der Gruppierung, etwa
`ujust setup-backup`, funktionieren weiterhin.

Die Befehle sind `just`-Rezepte, und `just` selbst steht Ihnen für Ihre eigenen Projekte zur Verfügung:
Ein `justfile` in einem beliebigen Ordner macht dessen Befehle auf dieselbe Weise zu Rezepten.

## Befehlszeilenwerkzeuge

Befehlszeilenwerkzeuge kommen von Homebrew, das ohne `sudo` in Ihren persönlichen Ordner installiert,
ohne das System anzurühren:

```bash
brew install ripgrep
brew search <name>
brew upgrade
```

Was es gibt, finden Sie unter [formulae.brew.sh](https://formulae.brew.sh). Homebrew wird zusammen mit dem
System aktualisiert. Führen Sie es nie mit `sudo` aus: Es gehört Ihnen, nicht root.

Bereits im Abbild: `just`, `gum`, `glow`, `tmux`, `fastfetch`, `git` und die übrigen üblichen Werkzeuge.
Zu den Schriften gehören Inter, JetBrains Mono und die Symbole der Nerd Fonts, sodass Prompts und
Dateimanager im Terminal ihre Symbole zeichnen können.
