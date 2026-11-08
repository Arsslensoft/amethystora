# Häufige Probleme

## Ein Paket, das ich überlagert habe, ist weg

Es wurde mit `rpm-ostree install` überlagert, und der Rechner wurde danach mit `bootc switch` gewechselt,
das das neue System allein aus dem Abbild baut. Installieren Sie es erneut, und wechseln Sie Abbilder oder
Kanäle ab jetzt mit `rpm-ostree rebase` ([Software installieren](software.md#layering-the-last-resort)).

## Eine App sieht einen Ordner oder ein Gerät nicht, oder öffnet sich gar nicht

Flatpak-Apps erreichen nur, was ihnen erlaubt ist, und unter Amethystora schließt das X11 und
Eingabegeräte aus. Öffnen Sie **Flatseal**, wählen Sie die App und gewähren Sie, was sie braucht: einen
Ordner unter **Dateisystem** oder **X11-Fenstersystem** für eine App, die kein Wayland spricht.

## Eine Erweiterung benimmt sich daneben, oder der Desktop sieht falsch aus

Schalten Sie die Erweiterung in der **Erweiterungsverwaltung** aus, und melden Sie sich dann ab und wieder
an. Wenn der Desktop sich weiterhin daneben benimmt, setzen Sie GNOMEs Einstellungen auf die Vorgaben des
Abbilds zurück und wenden Sie Ihr Thema erneut an:

```bash
dconf reset -f /org/gnome/
amethystora-theme reload
```

Das setzt jede GNOME-Einstellung dieses Kontos zurück, einschließlich des Docks, Ihrer eigenen
Tastenkürzel und Ihrer Auswahl an Erweiterungen; heben Sie es sich also für den Fall auf, dass nichts
anderes geholfen hat.

## `Super+Ctrl+Space` ändert nichts

Es wechselt zum nächsten Hintergrundbild des aktuellen Themas. Ein eigenes Thema mit nur einem Bild hat
nichts, wohin es wechseln könnte, und eine Benachrichtigung sagt das: Fügen Sie weitere hinzu
([Themen](wallpapers.md)). Die Themenauswahl liegt auf `Super+Ctrl+Shift+Space`.

## Das Konto ist nach falschen Passwörtern gesperrt

Zehn falsche Passwörter sperren das Konto für zehn Minuten. Warten Sie, oder entsperren Sie es von einem
anderen Administratorkonto aus mit `sudo faillock --user <name> --reset`.

## Ein DisplayLink-Dock zeigt nichts an

Bei eingeschaltetem Secure Boot lädt sein Treiber erst, wenn der Schlüssel von Amethystora registriert ist:
`ame security secure-boot`, dann neu starten ([Hardware](hardware.md#secure-boot)).

## Tastaturbelegungen wechseln

`Super+Space` ist hier die Suche, deshalb liegt die nächste Tastaturbelegung auf `Super+Shift+Space`.
Fügen Sie Belegungen unter **Einstellungen → Tastatur** hinzu.

## Die Festplatte läuft voll

```bash
ame system clean
```

Entfernt ungenutzte Container, Abbilder und Flatpak-Laufzeitumgebungen. `flatpak uninstall --unused` und
`podman system prune` erledigen Teile davon von Hand.
