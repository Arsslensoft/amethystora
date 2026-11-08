# Premiers pas

Quelques minutes maintenant, et la machine prend soin d’elle-même ensuite. Aucune de ces étapes n’est
obligatoire, mais chacune vaut la peine d’être faite une fois.

## 1. Faire reconnaître l’image par Secure Boot

Si Secure Boot est activé, les modules du noyau intégrés à l’image (stations d’accueil DisplayLink,
caméras virtuelles) ne se chargent qu’une fois leur clé de signature enregistrée. Le premier démarrage
vous l’a peut-être déjà demandé ; sinon, exécutez :

```bash
ame security secure-boot
```

Au redémarrage suivant, un écran bleu apparaît : le gestionnaire MOK. Choisissez **Enroll MOK**, puis
**Continue**, puis **Yes**, et saisissez le mot de passe `amethystora`. Cet écran utilise une
disposition de clavier américaine (QWERTY) : sur un clavier AZERTY, tapez `q,ethystorq`.
[Matériel](hardware.md#secure-boot) donne les détails.

## 2. Vérifier que tout est activé

```bash
ame security status
```

Un seul écran qui montre ce que font réellement les paramètres de sécurité de la machine, et que faire
pour tout ce qui est désactivé. Il ne modifie rien et ne demande aucun mot de passe. Le même rapport se
trouve dans la grille d’applications sous le nom **Sécurité**, qui recherche aussi les virus
([Sécurité](viruses.md#scanning-for-viruses)).

## 3. Installer vos applications

Les applications fournies avec Amethystora (Calculatrice, Éditeur de texte, Gestionnaire d’extensions
et les autres) s’installent d’elles-mêmes la première fois que la machine est en ligne, et apparaissent
dans la grille d’applications au fur et à mesure. Pour les avoir tout de suite, exécutez
`ame apps flatpaks`. Désinstallez celles dont vous ne voulez pas ; elles restent désinstallées.

Ouvrez ensuite **Bazaar**, la logithèque, et installez depuis Flathub ce que vous utilisez. Le
navigateur est Firefox ; si vous préférez Chrome ou Brave, ils sont aussi dans Bazaar. [Installer des
logiciels](software.md) traite des outils en ligne de commande et de tout le reste.

Vous venez d’une autre machine Amethystora qui était sauvegardée ? `ame restore-setup` remet en place
ses applications, ses conteneurs, ses extensions et son thème à partir de la sauvegarde
([Sauvegardes](backups.md)).

## 4. L’adapter à vos goûts

- `Super+Ctrl+Shift+Space` choisit un thème. Il repeint d’un coup tout le bureau, le terminal et
  l’invite de commande. [Thèmes](themes.md).
- Les fenêtres se placent toutes seules en mosaïque, côte à côte sur une bande qui défile. Ouvrez deux
  ou trois applications et passez de l’une à l’autre avec `Super+Left` et `Super+Right`.
  [Fenêtres en mosaïque](tiling.md). Si vous préférez des fenêtres qui flottent et se
  chevauchent, `ame desktop layout classic` passe à ce mode
  ([Fenêtres classiques](tiling.md#classic-windows)).
- `Super+Alt+Space` ouvre le menu Amethystora, qui donne accès au clavier à tout ce que décrit ce
  manuel.

## 5. Mettre en place une sauvegarde

```bash
ame backup
```

Une sauvegarde quotidienne vers un endroit dont un rançongiciel présent sur cette machine ne peut rien
supprimer. La commande vous guide pour choisir où. [Sécurité](backups.md).

## 6. Apprendre les raccourcis

La page [Raccourcis clavier](../keybindings.md) vaut la lecture. Gardez-la ouverte sur un deuxième
espace de travail pendant votre première semaine.

## Ensuite, n’y pensez plus

Les mises à jour arrivent toutes seules et s’appliquent au prochain redémarrage. Éteignez la machine
quand vous avez fini votre journée, et elle reste à jour sans que vous ayez à y penser.
[Mises à jour](updates.md) explique ce qui se passe et comment en annuler une.
