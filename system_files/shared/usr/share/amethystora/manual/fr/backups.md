# Sauvegardes

```bash
ame backup
```

Une sauvegarde restic quotidienne vers un dépôt dont un rançongiciel présent sur cette machine ne peut
rien supprimer. La sauvegarde ne fait qu’ajouter au dépôt et ne supprime jamais les anciens
instantanés : un dépôt en ajout seul à l’autre bout conserve donc tout ce qui a été sauvegardé avant
une attaque. La commande explique comment en mettre un en place sur rest-server, Borg, un stockage
objet avec immuabilité, ou un disque USB que vous débranchez entre deux sauvegardes. `ame backup now`
sauvegarde immédiatement, et `ame backup snapshots` liste ce qui s’y trouve.

Chaque sauvegarde conserve aussi votre configuration à côté de vos fichiers : l’image et le flux que
vous suivez, vos applications Flatpak et leur provenance, vos paquets Homebrew, les conteneurs créés par
`amepkg` avec ce que vous y avez installé, les extensions GNOME que vous aviez activées et votre thème.
Aucun mot de passe, aucune clé ni aucun jeton n’y figure. Sur une nouvelle machine, faites pointer
`ame backup` vers le même dépôt avec le même mot de passe, puis :

```bash
ame restore-setup
```

La commande montre d’abord ce qu’elle ferait, et ne fait que les parties que vous choisissez : passer à
la même image, via `ame system rebase`, qui n’accepte que des images signées ; installer les
applications et les paquets Homebrew ; recréer les conteneurs et y réinstaller ce qu’ils contenaient ;
réactiver les extensions ; appliquer le thème. `ame restore-setup <snapshot>` choisit une configuration
plus ancienne. Vos fichiers eux-mêmes reviennent avec restic.
