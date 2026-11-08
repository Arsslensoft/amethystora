# Mises à jour

Les mises à jour sont automatiques et discrètes. Le système, vos applications Flatpak et vos outils
Homebrew sont vérifiés en arrière-plan et téléchargés pendant que vous travaillez. Le nouveau système
est préparé à côté de celui en cours d’exécution et prend le relais au prochain redémarrage, d’un seul
coup. Rien n’est jamais à moitié mis à jour : une mise à jour ne peut donc pas laisser la machine dans
un état où elle ne démarre plus.

> **La seule habitude à garder :** éteignez ou redémarrez quand vous avez fini votre journée. C’est à
> ce moment-là qu’une mise à jour préparée est appliquée.

## Mettre à jour maintenant

Ouvrez **Mises à jour du système** depuis la grille d’applications (ou **Check for updates** sous
**System** dans le menu Amethystora) et appuyez sur **Mettre à jour maintenant**. Cela met à jour le
système, vos applications Flatpak et vos outils Homebrew en une fois, et montre l’avancement de chacun,
sans demander de mot de passe. C’est la même mise à jour que celle qu’exécutent les mises à jour
automatiques : si l’une d’elles est déjà en cours, la fenêtre la suit au lieu d’en lancer une autre.
Vous pouvez fermer la fenêtre à tout moment ; la mise à jour continue.

Un nouveau système ne prend toujours le relais qu’au prochain redémarrage. Dès qu’il est prêt,
« Mises à jour du système » le signale et propose de redémarrer. La fenêtre affiche aussi la version en
cours d’exécution, celle qui attend le redémarrage et celle conservée pour
[revenir en arrière](rolling-back.md), et permet de désactiver ou de réactiver les mises à jour
automatiques.

Si vous avez des [conteneurs](containers.md#keeping-them-up-to-date) créés avec `amepkg`, **Mettre à jour
maintenant** les met à jour juste après le reste, sous **Conteneurs**. Un conteneur qui ne se met pas à
jour le signale à cet endroit, et la mise à jour du système compte tout de même comme réussie.

Dans un terminal, la même chose en une fois :

```bash
ame update
```

| Commande | Rôle |
| --- | --- |
| `ame update` | Tout mettre à jour maintenant |
| `amepkg upgrade-all` | Mettre à jour maintenant les conteneurs d’autres distributions |
| `ame system auto-updates` | Désactiver les mises à jour automatiques, ou les réactiver |
| `ame changelog` | Ce qui a changé dans les paquets depuis l’image que vous utilisez |
| `rpm-ostree status` | Le système en cours d’exécution, la mise à jour préparée et celui vers lequel revenir |
| `fwupdmgr get-updates` | Les mises à jour de micrologiciel de cette machine, de son fabricant |

Les notes de version de chaque version stable se trouvent sur
[GitHub](https://github.com/arsslensoft/amethystora/releases).

## Micrologiciels

Le micrologiciel de la machine et de ses périphériques (le micrologiciel UEFI, les stations d’accueil,
les SSD, les claviers, le lecteur d’empreintes) est mis à jour séparément du système, car une mise à
jour de micrologiciel peut exiger que la machine soit branchée et redémarrée. Chaque jour, fwupd
cherche de nouveaux micrologiciels sur LVFS, où les fabricants les publient, et la section
**Micrologiciels** de **Mises à jour** liste chaque périphérique concerné, de quelle version à quelle
version. **Mettre à jour les micrologiciels…** les installe dans un terminal, où fwupd dit ce qu’il
s’apprête à faire et demande avant de redémarrer la machine. **Vérifier maintenant** interroge de
nouveau LVFS sans attendre. Dans un terminal :

```bash
fwupdmgr get-updates      # what there is
fwupdmgr update           # install it
fwupdmgr security         # how well the firmware protects this machine
```

Le micrologiciel UEFI lui-même est écrit au prochain redémarrage, par le micrologiciel : laissez la
machine branchée jusqu’à ce qu’elle ait redémarré. Le rapport de sécurité affiche la note que fwupd
donne aux protections du micrologiciel, de HSI:0 à HSI:5 ; la note qu’une machine peut atteindre
dépend surtout de son fabricant.
