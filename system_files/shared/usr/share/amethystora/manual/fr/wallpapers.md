# Fonds d’écran

Chaque thème est fourni avec trois fonds d’écran de lieux imaginaires dessinés dans ses propres
couleurs, et passer à un thème met le premier sur le bureau : une ville néon sous la pluie pour Tokyo
Night, une aurore boréale au-dessus d’un lac de montagne pour Nord, des pins dans la brume pour
Everforest, un coucher de soleil sur le désert pour Gruvbox, des sommets sous une lune pastel pour
Catppuccin, une éclipse pour Matte Black, et ainsi de suite. `Super+Ctrl+Space` les fait défiler. Les
deux thèmes Amethystora s’ouvrent sur les facettes d’une pierre taillée, et ont aussi le champ de
cristaux et un ciel nocturne de fumée violette ; les Paramètres de GNOME les proposent sous Apparence.

Pour ajouter les vôtres, placez des images (JPG, PNG, WebP ou SVG) dans un dossier portant le nom du
thème. Elles viennent après les images propres au thème :

```bash
mkdir -p ~/.config/amethystora/backgrounds/amethystora
cp ~/Pictures/mountains.jpg ~/.config/amethystora/backgrounds/amethystora/
```

`amethystora-theme bg list` montre ce que le thème actuel fait défiler, et
`amethystora-theme bg set <picture>` en applique une directement.

## Transitions

Un nouveau thème ou un nouveau fond d’écran n’apparaît pas simplement. L’écran se fige un instant
pendant que tout change en dessous, puis le nouveau bureau s’ouvre en cercle à partir du pointeur, avec
une lueur de la couleur d’accentuation du thème le long du bord. Cela fonctionne de la même façon quelle
que soit la manière dont vous faites le changement : les raccourcis ci-dessus, les Paramètres de GNOME,
ou le bouton Style sombre des réglages rapides.

```bash
ame desktop transition         # pick one from a list
ame desktop transition wave    # or name it
```

| Transition | Aspect |
| --- | --- |
| `grow` | Un cercle s’ouvre à partir du pointeur. Par défaut |
| `outer` | Le nouveau bureau se referme sur le pointeur depuis les bords |
| `wipe` | Un bord adouci balaie l’écran en biais |
| `wave` | La même chose, avec un bord ondulé |
| `fade` | Un simple fondu enchaîné |
| `random` | L’une des quatre premières, sous n’importe quel angle |
| `none` | Aucune animation : le fond d’écran s’estompe comme GNOME le fait |

Une transition dure 1,2 seconde. Pour changer cela, indiquez une durée en millisecondes :

```bash
gsettings set org.gnome.shell.extensions.amethystora-transitions duration 800
```

Il n’y a pas de transitions tant que les animations sont désactivées dans Paramètres, sous
Accessibilité. Elles viennent de l’extension Amethystora Transitions, que le Gestionnaire d’extensions
peut désactiver.
