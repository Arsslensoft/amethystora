# Dépannage

La plupart des problèmes sur un système à base d’image ont les trois mêmes réponses : découvrir ce qui
s’est passé, revenir en arrière si c’est une mise à jour qui l’a causé, et le signaler pour que la
prochaine image le corrige.

## Laisser l’agent regarder d’abord

**Diagnose a problem** dans le menu Amethystora (`Super+Alt+Space`), ou depuis un terminal :

```bash
amethystora-agent diagnose "the second monitor stays black after suspend"
```

Il lit les journaux et les rapports de plantage, ne modifie rien, et revient avec la cause et une
correction à envisager. [L’agent IA](ai-agent.md#diagnose-a-problem) en dit plus.

## Une mise à jour a cassé quelque chose

Choisissez le système précédent au menu de démarrage pour confirmer que c’est bien la mise à jour.
« Mises à jour du système » propose alors de le garder, ce que fait aussi `sudo bootc rollback`, jusqu’à
ce qu’une image corrigée soit sortie. [Mises à jour](rolling-back.md).

## Signaler un problème

Signalez les bogues sur [GitHub](https://github.com/arsslensoft/amethystora/issues), et posez vos questions
dans les [discussions](https://github.com/arsslensoft/amethystora/discussions). `ame report` rassemble ce
dont un rapport a besoin dans un fichier de votre dossier personnel (l’image, vos groupes, les services
en échec et les erreurs journalisées depuis ce démarrage), vous montre tout, puis, si vous le voulez,
ouvre un nouveau ticket avec tout cela déjà rempli, sauf les erreurs. Rien n’est publié tant que vous
ne l’envoyez pas ; faites glisser le fichier dans le ticket si les erreurs peuvent aider. Sinon,
indiquez :

- la sortie de `rpm-ostree status`, qui nomme l’image exacte,
- ce que vous avez fait, ce que vous attendiez, et ce qui s’est passé à la place,
- les lignes pertinentes des journaux ci-dessus,
- si l’image précédente avait le même problème.
