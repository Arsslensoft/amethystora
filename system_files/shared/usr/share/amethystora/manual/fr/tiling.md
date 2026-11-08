# Fenêtres en mosaïque

PaperWM range les fenêtres pour vous. Chaque espace de travail est une bande de fenêtres placées côte
à côte sur toute la hauteur, et la bande peut être plus large que l’écran. Une nouvelle fenêtre s’ouvre
à droite de celle où vous êtes, les fenêtres ne se chevauchent jamais, et la bande défile pour garder
la fenêtre active visible :

```
  ┌────────┬──────────────┬──────────────────────┬────────┐
  │  Mail  │   Terminal   │       Browser        │  Chat  │
  └────────┴──────────────┴──────────────────────┴────────┘
           ╰──────────── the screen ─────────────╯
```

Mail et Chat sont toujours ouverts, juste au-delà du bord. `Super+Left` revient à Mail. Poussez le
pointeur contre le bord gauche ou droit de l’écran pour voir ce qu’il y a au-delà, et cliquez sur une
fenêtre pour y aller. Sur un pavé tactile, balayez avec trois doigts pour faire glisser la bande.

Les raccourcis suivent une même logique. `Super` et une flèche passe à une autre fenêtre. Ajoutez
`Ctrl` et c’est la fenêtre qui se déplace. Ajoutez `Shift` et vous passez à un autre écran.

## Se déplacer

| Touche | Rôle |
| --- | --- |
| `Super+Left` / `Super+Right` | La fenêtre à gauche / à droite |
| `Super+Up` / `Super+Down` | La fenêtre au-dessus / en dessous, quand des fenêtres partagent une colonne |
| `Super+Home` / `Super+End` | La première / dernière fenêtre de l’espace de travail |
| `Super+[` / `Super+]` | Faire glisser la bande sans changer de fenêtre active |
| `Super+Shift` et une flèche | L’écran dans cette direction |

## Disposer

| Touche | Rôle |
| --- | --- |
| `Super+Ctrl+Left` / `Super+Ctrl+Right` | Déplacer la fenêtre vers la gauche / la droite le long de la bande |
| `Super+Ctrl+Up` / `Super+Ctrl+Down` | La déplacer vers le haut / le bas dans sa colonne |
| `Super+I` | Faire entrer la fenêtre de droite dans cette colonne, sous celle-ci |
| `Super+O` | Sortir la fenêtre du bas de cette colonne dans une colonne à part |
| `Super+Shift+O` | Sortir cette fenêtre dans une colonne à part |
| `Super+Ctrl+Page Up` / `Page Down` | La déplacer vers l’espace de travail précédent / suivant |
| `Super+Shift+Ctrl` et une flèche | La déplacer vers l’écran dans cette direction |
| `Super+T`, en maintenant `Super` | Emporter la fenêtre pendant que vous vous déplacez, et la déposer là où vous relâchez |

## Dimensionner

| Touche | Rôle |
| --- | --- |
| `Super+R` | Faire passer la largeur par 38 %, 50 % et 62 % de l’écran (`Super+Alt+R` revient en arrière) |
| `Super+Shift+R` | La même chose pour la hauteur d’une fenêtre dans une colonne |
| `Super+F` | Occuper toute la largeur de l’écran, et revenir |
| `Super+C` | Centrer la fenêtre à l’écran |
| `Super+Shift+C` | Où la bande amène une fenêtre active : juste dans le champ, au centre ou au bord |
| `Super+Shift+W` | Où s’ouvrent les nouvelles fenêtres : à droite, à gauche ou sous celle-ci |

## Fenêtres flottantes

Les boîtes de dialogue flottent d’elles-mêmes au-dessus de la bande. `Super+Ctrl+Escape` sort la
fenêtre active de la bande pour qu’elle flotte aussi, et l’y remet. `Super+Alt+Escape` masque d’un coup
toutes les fenêtres rendues flottantes de cette façon, et les fait réapparaître.

## Et le reste

| Touche | Rôle |
| --- | --- |
| `Super+N` | Une nouvelle fenêtre de l’application en cours |
| `Super+Ctrl+B` | Masquer ou afficher la barre supérieure sur cet espace de travail |

Les paramètres de PaperWM se trouvent sous **PaperWM** dans le **Gestionnaire d’extensions**. On y
trouve les espacements, les largeurs que parcourt `Super+R`, et tous les raccourcis, dont certains que
cette page ne mentionne pas.

## Fenêtres classiques

Si vous préférez des fenêtres qui flottent et se chevauchent, comme sous Windows et macOS, passez à la
disposition classique :

```bash
ame desktop layout classic
```

Elle désactive PaperWM, et les mises à jour le laissent désactivé. Les fenêtres s’ouvrent alors là où
GNOME les place, et vous les déplacez par leur barre de titre. `Super+Up` maximise et `Super+Down`
restaure, `Super+Left` / `Super+Right` placent une fenêtre sur cette moitié de l’écran, et
`Super+Shift+Left` / `Super+Shift+Right` la déplacent vers l’écran suivant. `ame desktop layout tiling`
rétablit la bande. La première fois que vous vous connectez, une notification propose le même choix.
