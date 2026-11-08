# Ein Abbild wählen

## Ein Abbild wählen

| Abbild | Für |
| --- | --- |
| `ghcr.io/arsslensoft/amethystora` | Den Standard-Desktop |
| `ghcr.io/arsslensoft/amethystora-dx` | Entwickler: bringt zusätzlich Docker, Incus, libvirt, VSCodium und mehr ([Entwicklermodus](developer.md)) |
| `ghcr.io/arsslensoft/amethystora-nvidia-open` | Den Standard-Desktop mit NVIDIAs offenem Kernel-Treiber |
| `ghcr.io/arsslensoft/amethystora-dx-nvidia-open` | Den Entwicklermodus mit NVIDIAs offenem Kernel-Treiber |

Die ISOs installieren die Abbilder ohne NVIDIAs Treiber. Auf einem Rechner mit einer NVIDIA-Karte bietet
der erste Start an, zum passenden NVIDIA-Abbild zu wechseln ([Hardware](hardware.md#graphics)).

## Einen Kanal wählen

Jedes Abbild gibt es in drei Kanälen, festgelegt durch das Tag nach dem Doppelpunkt:

| Tag | Für |
| --- | --- |
| `stable` | Alle. Wöchentlich gebaut, mit dem Kernel auf dem Stand von Fedora CoreOS, der dem von Fedora etwas hinterherhinkt, damit Kernel-Regressionen vorher auffallen. |
| `latest` | Das Neueste, was Fedora hat, neu gebaut, sobald sich Amethystora ändert. |
| `beta` | Zum Testen dessen, was als Nächstes kommt. Rechnen Sie mit Ecken und Kanten. |

Die ISOs installieren `stable`. Sie können jederzeit zwischen Abbildern und Kanälen wechseln;
[Aktualisierungen](streams.md#switching-streams-and-images) zeigt, wie.
