# Fragen und Antworten

## Warum kann ich `dnf install` nicht verwenden?

Weil das System ein Abbild ist, und das Abbild ist auf jedem Rechner dasselbe. Das macht Aktualisierungen
sicher und Rollbacks sofort möglich. Apps kommen von Flathub, Werkzeuge von Homebrew und alles andere aus
einem Container; [Software installieren](software.md) zeigt, was wofür gedacht ist. In einem
Fedora-Container (`amepkg containers new --template fedora`) funktioniert `dnf install` genau so, wie Sie
es kennen.

## Kann ich einen anderen Browser verwenden?

Der Browser ist Firefox, genau so, wie Fedora ihn baut: Amethystora fügt ihm keine Einstellungen oder
Erweiterungen hinzu. Chrome, Brave und jeder andere Browser sind in Bazaar. Um einen zum Standard zu
machen, öffnen Sie **Einstellungen → Apps → Standard-Apps**; [Web-Apps](web-apps.md#which-browser-runs-them)
öffnen sich dann ebenfalls darin.

## Wo ist Brave?

Amethystora kam früher mit Brave. Es ist jetzt in Bazaar, wie die anderen Browser. Was Sie darin hatten,
liegt noch in `~/.config/BraveSoftware`, und Firefox kann es übernehmen: **Einstellungen → Allgemein →
Browser-Daten importieren**.

## Ist das Fedora?

Darunter ja: Amethystora basiert auf Fedoras atomarem Desktop und GNOME und folgt den Veröffentlichungen
von Fedora. Was es hinzufügt, ist der Desktop drumherum, die Härtung, die Werkzeuge und ein Abbild, das
getestet und signiert wird, bevor es Sie erreicht.

## Kann ich spielen?

Ja. `ame apps gaming` installiert Steam mit dem, was seine Spiele brauchen, und gewährt Steam das X11, über
das es zeichnet und das jeder anderen App verweigert wird. Viele Windows-Spiele laufen über Proton in
Steam. [Spiele und Android-Apps](games.md) sagt, was es einrichtet und wie Sie Heroic oder Lutris
hinzufügen.

## Kann ich Android-Apps ausführen?

Ja, in Waydroid: `ame apps android` richtet es ein. [Spiele und Android-Apps](games.md#android-apps).

## Kann ich Windows-Software ausführen?

Oft. Installieren Sie **Bottles** aus Bazaar, das Windows-Programme in eigenen Präfixen ausführt. Wine
braucht X11, gewähren Sie es Bottles also in Flatseal. Für den Rest funktioniert eine virtuelle
Windows-Maschine im [Entwickler-Abbild](developer.md) für fast alles.

## Wie führe ich ein AppImage aus?

Installieren Sie **Gear Lever** aus Bazaar. Es legt AppImages in die Anwendungsübersicht und hält sie
aktuell.

## Wie installiere ich ein `.deb` oder ein `.rpm`?

Öffnen Sie es in Dateien. Es kommt in einen Container, der für seine Art von Paket erstellt wird, nach
einer Virenprüfung und einem Blick darauf, woher es stammt; dieselbe App von Flathub wird, falls es sie
gibt, zuerst angeboten. [Eine Paketdatei](containers.md#a-package-file).

## Wo liegen meine Einstellungen?

In Ihrem persönlichen Ordner: `~/.config` und `~/.local` für Apps und den Desktop,
`~/.config/amethystora` für das Thema und die übrigen eigenen Einstellungen von Amethystora und
`~/.var/app` für jedes Flatpak. Sichern Sie Ihren persönlichen Ordner, und Sie haben alles gesichert, was
Ihnen gehört.

## Warum immer sechs Arbeitsflächen?

Damit eine Zahl immer denselben Ort meint. Mit Arbeitsflächen, die kommen und gehen, hängt `Super+3` davon
ab, was Sie vor einer Stunde geöffnet hatten; mit sechs festen sind der Browser auf 1 und das Terminal auf
2 immer genau dort, wo Ihre Finger sie erwarten.

## Warum kann ich keine Dateien in `/usr` ändern?

`/usr` ist das Abbild, absichtlich schreibgeschützt und mit jeder Aktualisierung als Ganzes ersetzt.
Einstellungen, die den ganzen Rechner betreffen, gehören nach `/etc`, wo Ihre Änderungen erhalten bleiben
und bei jeder Aktualisierung zusammengeführt werden. Einstellungen für Ihr Konto gehören in Ihren
persönlichen Ordner.

## Kann ich dem Abbild vertrauen?

Es wird öffentlich auf GitHub aus dem Quellcode in
[arsslensoft/amethystora](https://github.com/arsslensoft/amethystora) gebaut, mit einem Schlüssel signiert,
dessen öffentliche Hälfte in jedem Abbild mitgeliefert wird, und von Ihrem Rechner nur angenommen, wenn
diese Signatur stimmt.
