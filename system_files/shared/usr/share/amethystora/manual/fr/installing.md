# Installation

Téléchargez Amethystora, copiez-le sur une clé USB, démarrez l’ordinateur sur la clé et essayez-le :
rien ne change sur l’ordinateur tant que vous ne choisissez pas d’installer. L’installation prend
environ dix minutes et ne nécessite pas de réseau.

## Télécharger

| Téléchargement | Ce que c’est |
| --- | --- |
| [amethystora.iso](https://download.amethystora.org/amethystora.iso) | Amethystora sur une clé : essayez-le, puis installez-le depuis la clé. Commencez par celui-ci. |
| [amethystora-non-uefi.iso](https://download.amethystora.org/amethystora-non-uefi.iso) | L’installateur seul, qui démarre aussi les ordinateurs trop anciens pour l’UEFI |

Chacun pèse plusieurs gigaoctets. À côté de chaque ISO se trouve sa somme de contrôle, `.sha256`,
signée avec la clé qui sert à vérifier chaque mise à jour. Pour vérifier que ce que vous avez
téléchargé est bien ce qui a été construit :

```bash
curl -LO https://raw.githubusercontent.com/arsslensoft/amethystora/main/cosign.pub
curl -LO https://download.amethystora.org/amethystora.iso.sha256
curl -LO https://download.amethystora.org/amethystora.iso.sha256.bundle
cosign verify-blob --key cosign.pub --bundle amethystora.iso.sha256.bundle amethystora.iso.sha256
sha256sum -c amethystora.iso.sha256
```

Sous Windows, `certutil -hashfile amethystora.iso SHA256` affiche la somme de contrôle, à comparer avec
celle de `amethystora.iso.sha256`.

Sur un ordinateur doté d’une carte graphique NVIDIA, installez à partir de l’un ou l’autre : une
notification après le premier démarrage propose l’image avec le pilote NVIDIA
([Matériel](hardware.md#graphics)).

## Copier sur une clé USB

Utilisez une clé de 16 Go ou plus : tout ce qu’elle contient sera effacé.

- **Depuis Windows ou macOS :** Fedora Media Writer (choisissez **Sélectionner un fichier .iso**) ou
  balenaEtcher. Sous Windows, Rufus fonctionne aussi, en mode image DD.
- **Depuis Linux :** Impression, ou `dd` :

  ```bash
  sudo dd if=amethystora.iso of=/dev/sdX bs=4M status=progress oflag=sync
  ```

  où `/dev/sdX` est la clé, telle que `lsblk` la liste. Si vous vous trompez, `dd` efface un autre
  disque.

Une clé préparée avec Ventoy ne démarre l’ISO qu’en mode GRUB2 de Ventoy.

## Essayer

Branchez la clé et démarrez l’ordinateur dessus : pendant le démarrage, appuyez sur sa touche de menu
de démarrage (F12, F11, F10, F8 ou Échap sur la plupart des PC ; maintenez Option sur un Mac Intel) et
choisissez la clé. Secure Boot peut rester activé. Choisissez **Try or install Amethystora**, ou le
**basic graphics mode** si l’écran reste noir.

Amethystora démarre comme il le fera une fois installé, avec une session déjà ouverte. Le Wi-Fi, le
son, l’affichage et le reste du matériel montrent si cet ordinateur lui convient. Rien n’est écrit sur
le disque de l’ordinateur, et tout ce que vous faites disparaît à l’extinction. Les applications de
Flathub ne sont pas sur la clé : elles s’installent au premier démarrage du système installé.

L’ISO live ne démarre que les ordinateurs dotés d’un micrologiciel UEFI, ce qu’ont presque tous les
ordinateurs depuis 2012. Pour un ordinateur plus ancien, utilisez l’installateur seul.

## Installer

Choisissez **Install to Hard Drive** dans le dock. L’installateur demande votre langue et le disque,
puis fait le reste.

- **Choisissez le chiffrement du disque.** Il ne peut pas être activé après coup, et c’est lui qui
  protège vos fichiers si l’ordinateur est perdu ou volé. La phrase de passe est demandée à chaque
  démarrage, jusqu’à ce que [le TPM déverrouille le disque](disk-encryption.md#unlock-the-disk-with-the-tpm),
  si vous le souhaitez.
- **Pour garder Windows,** lisez d’abord [À côté de Windows](#next-to-windows).

Une fois l’installation terminée, redémarrez et retirez la clé. Le premier démarrage demande votre
langue, votre clavier, votre compte et votre mot de passe, et les applications s’installent
d’elles-mêmes dès que l’ordinateur est en ligne. Suivez ensuite [Premiers pas](getting-started.md).

## À côté de Windows

Amethystora s’installe à côté de Windows, et un menu à chaque démarrage vous permet de choisir l’un ou
l’autre. Dans Windows, avant d’installer :

1. **Conservez votre clé de récupération BitLocker.** La plupart des portables sous Windows 11
   chiffrent leur disque avec BitLocker, aussi appelé chiffrement de l’appareil. Trouvez la clé dans
   **Paramètres › Confidentialité et sécurité › Chiffrement de l’appareil**, ou sur
   [aka.ms/myrecoverykey](https://aka.ms/myrecoverykey), et gardez-la ailleurs que sur cet
   ordinateur : l’installation ajoute une entrée au menu de démarrage de l’ordinateur, et Windows peut
   ensuite demander la clé une fois.
2. **Faites de la place.** Faites un clic droit sur **Démarrer**, ouvrez **Gestion des disques**,
   faites un clic droit sur la partition Windows (C:) et choisissez **Réduire le volume**. Laissez au
   moins 64 Go libres pour Amethystora, davantage si vous comptez y garder vos fichiers.
3. **Désactivez le démarrage rapide :** **Panneau de configuration › Options d’alimentation ›
   Choisir l’action des boutons d’alimentation**, et décochez **Activer le démarrage rapide**. Tant
   qu’il est activé, Windows ne s’éteint qu’à moitié et garde son disque verrouillé.

Ensuite, dans l’installateur, installez dans l’espace libre que vous avez créé, jamais sur le disque
entier. Laissez Secure Boot activé : Windows 11 en a besoin, et Amethystora démarre avec. Si
l’horloge de Windows est décalée de quelques heures après une utilisation d’Amethystora, activez
**Régler l’heure automatiquement** dans les paramètres de date et d’heure de Windows.

## Depuis un autre bureau Fedora Atomic

Un ordinateur qui exécute déjà Fedora Silverblue, ou un autre bureau construit sous forme d’image sur
Fedora, passe à Amethystora sans réinstallation. Le changement remplace l’image du système
d’exploitation et conserve tout ce qui se trouve dans `/home`, `/etc` et `/var`.

- **Sauvegardez** tout ce que vous ne pouvez pas perdre. Le changement est sans risque, mais une
  sauvegarde l’est toujours.
- Les paquets que vous avez superposés à l’ancien système avec `rpm-ostree install` ne suivent pas.
  Les Flatpak, Homebrew et les conteneurs, si.

```bash
sudo bootc switch ghcr.io/arsslensoft/amethystora:stable
```

Redémarrez ensuite. Au premier démarrage, un service prépare de lui-même le passage aux mises à jour
vérifiées par signature, et à partir du redémarrage suivant, une mise à jour n’est acceptée que si
elle porte la signature d’Amethystora.

## Après le premier démarrage

Suivez [Premiers pas](getting-started.md). La seule étape à ne pas sauter est l’enregistrement de la
clé de Secure Boot, si Secure Boot est activé.
