# Flux

| Étiquette | Ce que vous obtenez |
| --- | --- |
| `stable` | Construite chaque semaine, avec le noyau aligné sur celui de Fedora CoreOS, qui a un peu de retard sur celui de Fedora afin que les régressions du noyau soient repérées d’abord. Pour tout le monde. |
| `stable-daily` | La même, reconstruite à chaque changement d’Amethystora plutôt que chaque semaine |
| `latest` | Le plus récent de Fedora et du noyau, reconstruit à chaque changement d’Amethystora |
| `beta` | Ce qui arrive ensuite. Attendez-vous à quelques aspérités. |

## Changer de flux et d’image

Pour passer à un autre flux ou à une autre image, changez et redémarrez :

```bash
sudo bootc switch --enforce-container-sigpolicy ghcr.io/arsslensoft/amethystora:latest
```

Remplacez `amethystora` par `amethystora-dx` pour le [mode développeur](developer.md), et `latest` par
le flux que vous voulez.

Si vous avez des [paquets superposés](software.md#layering-the-last-resort), changez plutôt avec
`rpm-ostree`, car `bootc switch` construit le nouveau système à partir de l’image seule et les
abandonnerait :

```bash
sudo rpm-ostree rebase ostree-image-signed:docker://ghcr.io/arsslensoft/amethystora:latest
```

## Mises à jour signées

Une mise à jour n’est acceptée que si elle porte la signature d’Amethystora, vérifiée avec la clé
`/etc/pki/containers/amethystora.pub`. Une image altérée, ou provenant de quelqu’un d’autre, est refusée
au lieu d’être installée. Un service vérifie à chaque démarrage que la vérification des signatures est
toujours activée, et la réactive si un changement d’image l’a désactivée.
