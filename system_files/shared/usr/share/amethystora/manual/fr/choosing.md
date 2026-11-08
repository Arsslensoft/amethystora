# Choisir une image

## Choisir une image

| Image | Pour |
| --- | --- |
| `ghcr.io/arsslensoft/amethystora` | Le bureau standard |
| `ghcr.io/arsslensoft/amethystora-dx` | Les développeurs : ajoute Docker, Incus, libvirt, VSCodium et plus encore ([Mode développeur](developer.md)) |
| `ghcr.io/arsslensoft/amethystora-nvidia-open` | Le bureau standard avec le pilote noyau ouvert de NVIDIA |
| `ghcr.io/arsslensoft/amethystora-dx-nvidia-open` | Le mode développeur avec le pilote noyau ouvert de NVIDIA |

Les ISO installent les images sans le pilote NVIDIA. Sur un ordinateur doté d’une carte NVIDIA, le
premier démarrage propose de passer à l’image NVIDIA correspondante ([Matériel](hardware.md#graphics)).

## Choisir un flux

Chaque image existe en trois flux, déterminés par l’étiquette qui suit les deux-points :

| Étiquette | Pour |
| --- | --- |
| `stable` | Tout le monde. Construite chaque semaine, avec le noyau aligné sur celui de Fedora CoreOS, qui a un peu de retard sur celui de Fedora afin que les régressions du noyau soient repérées d’abord. |
| `latest` | Le plus récent de Fedora, reconstruit à chaque changement d’Amethystora. |
| `beta` | Pour tester ce qui arrive. Attendez-vous à quelques aspérités. |

Les ISO installent `stable`. Vous pouvez passer d’une image ou d’un flux à l’autre à tout moment ;
[Mises à jour](streams.md#switching-streams-and-images) explique comment.
