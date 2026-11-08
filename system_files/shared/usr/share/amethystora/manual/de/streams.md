# Kanäle

| Tag | Was Sie bekommen |
| --- | --- |
| `stable` | Jede Woche gebaut, mit dem Kernel auf dem Stand von Fedora CoreOS, der dem von Fedora etwas hinterherhinkt, damit Kernel-Regressionen vorher auffallen. Für alle. |
| `stable-daily` | Dasselbe, aber neu gebaut, sobald sich Amethystora ändert, statt wöchentlich |
| `latest` | Das neueste Fedora und der neueste Kernel, neu gebaut, sobald sich Amethystora ändert |
| `beta` | Was als Nächstes kommt. Rechnen Sie mit Ecken und Kanten. |

## Kanäle und Abbilder wechseln

Um zu einem anderen Kanal oder Abbild zu wechseln, wechseln Sie dorthin und starten neu:

```bash
sudo bootc switch --enforce-container-sigpolicy ghcr.io/arsslensoft/amethystora:latest
```

Ersetzen Sie `amethystora` durch `amethystora-dx` für den [Entwicklermodus](developer.md) und `latest`
durch den gewünschten Kanal.

Wenn Sie [überlagerte Pakete](software.md#layering-the-last-resort) haben, wechseln Sie stattdessen mit
`rpm-ostree`, weil `bootc switch` das neue System allein aus dem Abbild baut und sie fallen lassen würde:

```bash
sudo rpm-ostree rebase ostree-image-signed:docker://ghcr.io/arsslensoft/amethystora:latest
```

## Signierte Aktualisierungen

Eine Aktualisierung wird nur angenommen, wenn sie die Signatur von Amethystora trägt, geprüft gegen den
Schlüssel in `/etc/pki/containers/amethystora.pub`. Ein manipuliertes Abbild oder eines von jemand anderem
wird abgelehnt statt installiert. Ein Dienst prüft bei jedem Start, ob die Signaturprüfung noch
eingeschaltet ist, und schaltet sie wieder ein, falls ein Wechsel sie ausgeschaltet hat.
