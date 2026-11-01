# Mode développeur

`amethystora-dx` est le même bureau avec un atelier de développeur par-dessus : moteurs de conteneurs,
machines virtuelles, VSCodium, et les outils de profilage pour découvrir pourquoi quelque chose est
lent.

L’idée qui le sous-tend : votre environnement de développement ne devrait pas être votre système
d’exploitation. Les chaînes de compilation, les environnements d’exécution des langages et les bases de
données vivent dans des conteneurs décrits dans le dépôt même du projet : le projet se construit donc
de la même façon sur votre machine, sur le Mac d’un collègue et en CI, et le système en dessous reste
assez propre pour être mis à jour sans arrière-pensée.

## L’activer

Passez à l’image dx, et redémarrez :

```bash
sudo bootc switch --enforce-container-sigpolicy ghcr.io/arsslensoft/amethystora-dx:stable
```

Si vous avez des [paquets superposés](software.md#layering-the-last-resort), utilisez plutôt
`sudo rpm-ostree rebase ostree-image-signed:docker://ghcr.io/arsslensoft/amethystora-dx:stable`, qui les
conserve. Revenir à `amethystora` se fait de la même façon.

Au premier démarrage, l’image dx ajoute chaque compte administrateur aux groupes `docker`,
`incus-admin`, `libvirt` et `wireshark`. Si `docker ps` répond « permission denied », déconnectez-vous
puis reconnectez-vous une fois.

## Ce qu’il ajoute

| Outil | Pour |
| --- | --- |
| VSCodium | L’éditeur : VS Code construit à partir de son code source ouvert, sans la télémétrie ni la licence de Microsoft. Les extensions viennent d’[Open VSX](https://open-vsx.org) ; Open Remote SSH et Container Tools sont installées. Ses couleurs suivent le [thème](themes.md). |
| Docker Engine | Avec buildx et compose. Le choix par défaut pour les dev containers. |
| Podman | Avec `podman-compose` et `podman machine`. Toujours présent, sans root, et le socket Podman est activé. |
| Incus | Des conteneurs système et des machines virtuelles, gérés comme des instances cloud |
| libvirt et virt-manager | Des machines virtuelles avec QEMU et KVM |
| Sysprof, perf, bcc, bpftrace, bpftop, bpftool, trace-cmd | Profilage et traçage, de tout le système jusqu’à une seule fonction |
| gdb, strace, ltrace, Valgrind | Débogage : exécuter un programme pas à pas, observer les appels qu’il fait au noyau et à ses bibliothèques, trouver les erreurs de mémoire. Attacher gdb à un programme qu’il n’a pas lancé nécessite `sudo` ([pourquoi](security.md#on-from-the-start)). |
| Wireshark | Capturer et lire le trafic réseau, sans `sudo` |
| android-tools | `adb` et `fastboot` |
| ROCm | Le calcul GPU sur les cartes graphiques AMD |
| flatpak-builder | La construction de Flatpak |

## Dev Containers

Un dev container est un fichier `.devcontainer/devcontainer.json` dans le projet : l’image, les outils
et les extensions d’éditeur dont le projet a besoin. Tout ce que le projet installe reste là-dedans.

L’extension Dev Containers de Microsoft ne fonctionne que dans le VS Code de Microsoft : sur
Amethystora, les dev containers passent donc par la ligne de commande `devcontainer`, l’implémentation
de référence de [la spécification](https://containers.dev), que suivent aussi les IDE JetBrains :

```bash
brew install devcontainer
devcontainer up --workspace-folder .
devcontainer exec --workspace-folder . bash
```

Pour utiliser Podman au lieu de Docker, ajoutez
`--docker-path podman --docker-compose-path podman-compose` à `devcontainer up`.

Si un conteneur ne peut pas lire un dossier monté à cause de SELinux, réétiquetez le dossier plutôt que
de désactiver SELinux : `restorecon -R -v ~/code/myproject`.

## Autres éditeurs

- **JetBrains :** `ame apps jetbrains-toolbox` installe la JetBrains Toolbox dans votre dossier
  personnel, qui installe ensuite les IDE et les met à jour. Les Flatpak des IDE ne sont pas
  recommandés.
- **Neovim, Helix et compagnie :** `brew install neovim`, et `brew install devcontainer` pour la ligne
  de commande des dev containers.

## Kubernetes et le cloud

Installez les outils en ligne de commande avec Homebrew, pour qu’ils restent à jour sans toucher à
l’image :

```bash
brew install kubectl helm k9s kind
```

`kind` exécute tout un cluster Kubernetes dans des conteneurs Docker : c’est le moyen le plus rapide
d’essayer quelque chose sur un vrai cluster.
