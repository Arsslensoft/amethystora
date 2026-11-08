# Modifier un thème, ou créer le vôtre

Les thèmes sont fournis en lecture seule dans `/usr/share/amethystora/themes`. Un dossier du même nom
dans `~/.config/amethystora/themes` vient s’y superposer : vous n’écrivez donc que les fichiers que vous
voulez changer. Un dossier qui porte un nom à lui est un nouveau thème. Chaque fichier est petit et
facultatif, sauf la palette :

| Fichier | Contient |
| --- | --- |
| `colors.toml` | La palette : `accent`, `foreground`, `background`, `cursor`, `selection_foreground`, `selection_background`, et `color0` à `color15` |
| `light.mode` | Présent, et vide, quand le thème est clair |
| `pair.theme` | Le thème vers lequel bascule `Super+Ctrl+D` |
| `accent.theme` | La couleur d’accentuation de GNOME : blue, teal, green, yellow, orange, red, pink, purple ou slate. Sans lui, la plus proche est choisie |
| `gtk.theme` | Un nom de thème GTK, au lieu de l’apparence propre à GNOME |
| `icons.theme` | Un thème d’icônes, au lieu de candy-icons |
| `cursor.theme` | Un thème de curseur |
| `vscode.theme` | Le thème de couleurs de VSCodium à utiliser |
| `tophat.theme` | La couleur des indicateurs de la barre supérieure, quand l’accentuation passe mal en trait fin |
| `backgrounds/` | Les fonds d’écran |
| `backgrounds.list` | Des fonds d’écran conservés ailleurs sur le système, un chemin par ligne. Le premier est celui sur lequel le thème s’ouvre |

Un thème sans images propres en reçoit quatre dessinées dans ses couleurs : une lueur douce, des
collines dans la brume, une carte en courbes de niveau et des rubans ondoyants.

Un thème à vous tient en trois commandes :

```bash
mkdir -p ~/.config/amethystora/themes/sunset
cp /usr/share/amethystora/themes/amethystora/colors.toml ~/.config/amethystora/themes/sunset/
amethystora-theme set sunset
```

Modifiez les couleurs, relancez `amethystora-theme set sunset`, et regardez le résultat.

## Aller plus loin

- **La façon dont une configuration est écrite.** La palette du terminal, l’invite, les couleurs de
  btop et les fonds d’écran dessinés sont des modèles situés dans `/usr/share/amethystora/themed`.
  Copiez-en un dans `~/.config/amethystora/themed/` et modifiez la copie pour changer la façon dont
  chaque thème le rend.
- **Lancer quelque chose à chaque changement.** Placez un exécutable dans
  `~/.config/amethystora/hooks/theme-set.d/`. Il reçoit le nom du thème dans `AMETHYSTORA_THEME` et son
  dossier dans `AMETHYSTORA_THEME_DIR`.
- **Votre propre invite reste la vôtre.** Si vous avez écrit vous-même `~/.config/starship.toml`, le
  thème n’y touche pas.
- **Ne modifiez jamais `~/.config/amethystora/current/`.** Il est réécrit à chaque changement.

## Le menu de démarrage

Le menu de démarrage peut aussi porter les illustrations d’Amethystora. C’est désactivé par défaut,
car le menu de démarrage se trouve hors de l’image :

```bash
ame desktop boot-menu
```

Relancez la commande pour retirer les illustrations. Une fois activées, les mises à jour de l’image les
tiennent à jour.
