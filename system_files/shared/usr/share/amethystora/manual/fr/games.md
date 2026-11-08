# Jeux et applications Android

Ni les uns ni les autres ne sont configurés d’emblée, car chacun a besoin de quelque chose que l’image
retire aux applications qui n’en ont pas besoin. Une commande pour chacun les configure, dit d’abord ce
qu’elle change, et peut tout retirer ensuite.

## Jeux

```bash
ame apps gaming
```

Cette commande installe **Steam** depuis Flathub, avec **ProtonPlus**, qui ajoute d’autres versions de
Proton, et deux outils que les jeux Steam peuvent utiliser : la surcouche **MangoHud** et le
compositeur **gamescope** de Valve. Elle propose aussi **Heroic**, pour les boutiques Epic, GOG et
Amazon, et **Lutris**, pour les jeux venant d’ailleurs.

Tout Flatpak se voit refuser X11 sur ce bureau, car une application sous X11 peut lire ce que vous
tapez dans d’autres applications X11
([ce que les applications Flatpak n’ont pas le droit de faire](software.md#what-flatpak-apps-may-not-do)).
Steam et les jeux Windows qu’il exécute via Proton n’affichent rien autrement : X11 est donc rendu à
Steam et aux lanceurs que vous ajoutez, pour votre compte seulement. La page **Applications** de
l’application Sécurité le liste avec toutes les autres permissions
([Permissions des applications](app-permissions.md)).

Dans les **Propriétés** d’un jeu dans Steam, sous **Options de lancement** :

| Option de lancement | Rôle |
| --- | --- |
| `mangohud %command%` | La surcouche : images par seconde, temps d’image, températures |
| `gamemoderun %command%` | Le mode performance du processeur pendant le jeu, via GameMode |
| `gamescope -f -- %command%` | Le jeu dans son propre compositeur, en plein écran, ce qui aide avec les jeux qui n’aiment pas Wayland |

Les manettes fonctionnent sans rien configurer : Xbox, PlayStation, Nintendo et Steam, 8BitDo, et
toutes les autres que Steam connaît. **Add another launcher** ajoute Heroic ou Lutris plus tard, et
**Take it all away** les désinstalle tous, avec leurs jeux, et retire ce qui avait été accordé :

```bash
ame apps gaming launchers
ame apps gaming off
```

## Un ordonnanceur de processeur plus rapide pour les jeux

Le noyau décide quel programme s’exécute sur quel cœur du processeur, et pendant combien de temps. Son
propre ordonnanceur convient à presque tout. Un ordonnanceur sched-ext est un petit programme auquel le
noyau confie cette tâche, et un ordonnanceur conçu pour un type de travail peut mieux la faire :

```bash
ame system scheduler
```

| Ordonnanceur | Pour |
| --- | --- |
| `kernel` | Tout : celui du noyau, et celui par défaut |
| `lavd` | Les jeux, fluides pendant que d’autres tâches tournent : conçu pour les consoles portables et les portables de jeu |
| `bpfland` | Un bureau qui reste réactif pendant une compilation ou un rendu lourd |
| `flash` | Une synchronisation régulière, pour l’audio et la musique |

Le choix est conservé après un redémarrage, et `ame system scheduler kernel` remet l’ordonnanceur du
noyau. Si un ordonnanceur sched-ext s’arrête, le noyau reprend la main de lui-même : le pire qu’il
puisse faire est donc de ralentir les choses. Un noyau construit avant dwarves 1.32 de Fedora ne peut
pas charger les ordonnanceurs actuels ; la commande le signale et laisse en place celui du noyau.

## Applications Android

```bash
ame apps android
```

**Waydroid** exécute Android, LineageOS 20, dans un conteneur à côté de votre bureau, et les
applications Android que vous installez apparaissent dans la grille d’applications comme les autres.
Sa configuration télécharge Android depuis les serveurs de Waydroid, environ un gigaoctet. Il est
fourni sans les applications de Google ni le Play Store, dont les applications sont soumises aux
conditions propres de Google : prenez vos applications sur F-Droid, ou installez un `.apk` que vous
avez téléchargé :

```bash
waydroid app install ~/Downloads/some-app.apk
```

Ouvrez **Waydroid** depuis la grille d’applications ; le premier démarrage prend une minute. Son
service de conteneur s’exécute en tant que root : il n’est donc activé qu’une fois que vous l’avez
configuré, et vous pouvez l’arrêter quand vous ne l’utilisez pas :

| Commande | Rôle |
| --- | --- |
| `ame apps android start` | Démarrer Android, et l’ouvrir |
| `ame apps android stop` | L’arrêter, jusqu’à ce que vous le redémarriez |
| `ame apps android off` | Supprimer Android, avec toutes les applications et tout ce qui y est enregistré |

Waydroid ne peut pas dessiner avec le pilote NVIDIA : sur les images NVIDIA, Android dessine donc en
logiciel, ce qui convient à la plupart des applications, mais reste lent pour les jeux.
