# Problèmes courants

## Un paquet que j’avais superposé a disparu

Il avait été superposé avec `rpm-ostree install`, puis la machine a changé d’image avec `bootc switch`,
qui construit le nouveau système à partir de l’image seule. Installez-le de nouveau, et changez
désormais d’image ou de flux avec `rpm-ostree rebase`
([Installer des logiciels](software.md#layering-the-last-resort)).

## Une application ne voit pas un dossier, un périphérique, ou ne s’ouvre pas du tout

Les applications Flatpak n’accèdent qu’à ce qui leur est autorisé, et sur Amethystora cela exclut X11
et les périphériques d’entrée. Ouvrez **Flatseal**, choisissez l’application, et accordez-lui ce dont
elle a besoin : un dossier sous **Filesystem**, ou **Système de fenêtrage X11** pour une
application qui ne parle pas Wayland.

## Une extension se comporte mal, ou le bureau a un aspect étrange

Désactivez l’extension dans le **Gestionnaire d’extensions**, puis déconnectez-vous et reconnectez-vous.
Si le bureau se comporte encore mal, réinitialisez les paramètres de GNOME aux valeurs par défaut de
l’image et réappliquez votre thème :

```bash
dconf reset -f /org/gnome/
amethystora-theme reload
```

Cela réinitialise tous les paramètres GNOME de ce compte, y compris le dock, vos raccourcis clavier
personnalisés et vos choix d’extensions : gardez-le pour quand rien d’autre n’a fonctionné.

## `Super+Ctrl+Space` ne change rien

Ce raccourci passe au fond d’écran suivant du thème actuel. Un thème à vous qui n’a qu’une seule image
n’a rien vers quoi passer, et une notification le signale : ajoutez-en d’autres
([Thèmes](wallpapers.md)). Le sélecteur de thème est `Super+Ctrl+Shift+Space`.

## Le compte est verrouillé après des mots de passe erronés

Dix mots de passe erronés verrouillent le compte pendant dix minutes. Attendez, ou déverrouillez-le
depuis un autre compte administrateur avec `sudo faillock --user <name> --reset`.

## Une station d’accueil DisplayLink n’affiche rien

Avec Secure Boot activé, son pilote ne se charge qu’une fois la clé d’Amethystora enregistrée :
`ame security secure-boot`, puis redémarrez ([Matériel](hardware.md#secure-boot)).

## Changer de disposition de clavier

`Super+Space` est la recherche ici : la disposition de clavier suivante est donc sur
`Super+Shift+Space`. Ajoutez des dispositions dans **Paramètres → Clavier**.

## Le disque se remplit

```bash
ame system clean
```

Supprime les conteneurs, images et environnements d’exécution Flatpak inutilisés.
`flatpak uninstall --unused` et `podman system prune` en font une partie à la main.
