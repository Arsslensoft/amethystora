# Virus et rançongiciels

## Rechercher des virus

**Analyse antivirus** dans l’application **Sécurité** (ou `amethystora-security scan`) analyse quand
vous le demandez :

- **Votre dossier personnel**, hormis les caches.
- **Un fichier ou un dossier** : choisissez-le, ou déposez-le n’importe où sur la fenêtre.
- **Toute la machine** : tous les dossiers personnels et temporaires, comme le fait l’analyse
  hebdomadaire. Elle ne demande pas de mot de passe et continue si vous fermez la fenêtre. Ce qu’elle
  trouve est conservé là où seul un administrateur peut le lire : le voir demande donc bien votre mot
  de passe.

Une analyse que vous lancez s’exécute en votre nom : elle vérifie donc ce que votre compte peut lire.
Par défaut, rien de ce qu’elle trouve n’est supprimé, mis en quarantaine ni déplacé : le résultat liste
chaque fichier, avec **Afficher dans Fichiers**, et vous laisse décider. Une détection peut être un faux
positif : vérifiez donc d’où vient un fichier avant de le supprimer, puis relancez l’analyse. Si vous
avez réglé les analyses pour mettre en quarantaine ou supprimer, l’application demande votre mot de
passe une fois l’analyse terminée et indique ce qu’il est advenu de chaque fichier. Les analyses que
vous avez lancées sont listées dans `~/.local/state/amethystora/security-scans.json`.

## Protection contre les rançongiciels

```bash
ame security ransomware
```

Toutes les heures, un instantané en lecture seule de chaque dossier personnel de cette machine, conservé
dans `/var/home/.snapshots`. Un rançongiciel qui s’exécute en votre nom peut chiffrer tout ce dans quoi
vous pouvez écrire, et un instantané n’est pas quelque chose dans quoi vous pouvez écrire : en modifier
ou en supprimer un exige un administrateur. Tous les instantanés de la dernière journée sont conservés,
puis un par jour pendant deux semaines. Le rapport le signale quand le plus récent date de plus de
quelques heures.

Pour récupérer des fichiers, exécutez `ame security ransomware restore` et choisissez le moment auquel
revenir. **Open it in Files** montre votre dossier personnel tel qu’il était alors, pour y copier des
fichiers ; **Put a folder back** recopie tout un dossier à sa place, en remplaçant les fichiers qu’il
contient et en laissant ceux qu’il ne contient pas, comme ceux qu’un rançongiciel a renommés. Les
fichiers des autres ne sont pas visibles dans un instantané, et les vôtres ne leur sont pas visibles.

Elle nécessite les dossiers personnels sur un sous-volume btrfs à part, ce que l’installateur met en
place sur un disque sauf s’il a été partitionné à la main. Le coût est l’espace disque : un instantané
retient ce qui a été modifié ou supprimé depuis, si bien qu’un fichier que vous supprimez pour faire de
la place ne libère son espace que lorsque le dernier instantané qui le contient disparaît, jusqu’à deux
semaines plus tard. Les instantanés ne sont jamais supprimés pour faire de la place, car un rançongiciel
qui réécrit chaque fichier supprimerait alors ceux d’avant son passage. La désactiver propose de les
supprimer.

Les instantanés se trouvent sur le même disque, et un rançongiciel qui obtient les droits
d’administrateur peut les supprimer. Gardez aussi une [sauvegarde](backups.md) ailleurs.
