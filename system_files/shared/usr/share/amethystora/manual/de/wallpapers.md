# Hintergrundbilder

Jedes Thema bringt drei Hintergrundbilder erfundener Orte mit, gemalt in seinen eigenen Farben, und der
Wechsel zu einem Thema legt dessen erstes auf den Desktop: eine Neonstadt im Regen für Tokyo Night, ein
Polarlicht über einem Bergsee für Nord, Kiefern im Nebel für Everforest, ein Sonnenuntergang in der Wüste
für Gruvbox, Gipfel unter einem Pastellmond für Catppuccin, eine Finsternis für Matte Black und so weiter.
`Super+Ctrl+Space` schaltet durch sie hindurch. Die beiden Amethystora-Themen beginnen mit den Facetten
eines geschliffenen Steins und haben außerdem das Kristallfeld und einen Nachthimmel aus violettem Rauch;
GNOMEs Einstellungen bieten diese unter „Darstellung“ an.

Um eigene hinzuzufügen, legen Sie Bilder (JPG, PNG, WebP oder SVG) in einen Ordner, der nach dem Thema
benannt ist. Sie folgen auf die eigenen Bilder des Themas:

```bash
mkdir -p ~/.config/amethystora/backgrounds/amethystora
cp ~/Pictures/mountains.jpg ~/.config/amethystora/backgrounds/amethystora/
```

`amethystora-theme bg list` zeigt, durch welche Bilder das aktuelle Thema schaltet, und
`amethystora-theme bg set <picture>` setzt eines direkt.

## Übergänge

Ein neues Thema oder Hintergrundbild erscheint nicht einfach. Der Bildschirm hält einen Moment still,
während sich darunter alles ändert, dann öffnet sich der neue Desktop in einem Kreis vom Mauszeiger aus,
mit einem Leuchten in der Akzentfarbe des Themas entlang des Randes. Es funktioniert gleich, egal auf
welchem Weg Sie die Änderung vornehmen: mit den Tasten oben, in GNOMEs Einstellungen oder mit dem Knopf
„Dunkler Stil“ in den Schnelleinstellungen.

```bash
ame desktop transition         # pick one from a list
ame desktop transition wave    # or name it
```

| Übergang | Sieht aus wie |
| --- | --- |
| `grow` | Ein Kreis öffnet sich vom Mauszeiger aus. Die Vorgabe |
| `outer` | Der neue Desktop schließt sich von den Rändern her um den Mauszeiger |
| `wipe` | Eine weiche Kante wischt schräg über den Bildschirm |
| `wave` | Dasselbe, mit einer welligen Kante |
| `fade` | Eine einfache Überblendung |
| `random` | Einer der ersten vier, in einem beliebigen Winkel |
| `none` | Keine Animation: Das Hintergrundbild blendet über, wie GNOME es überblendet |

Ein Übergang dauert 1,2 Sekunden. Um das zu ändern, geben Sie die Dauer in Millisekunden an:

```bash
gsettings set org.gnome.shell.extensions.amethystora-transitions duration 800
```

Solange Animationen in den Einstellungen unter Barrierefreiheit ausgeschaltet sind, gibt es keine
Übergänge. Sie stammen von der Erweiterung Amethystora Transitions, die sich in der
Erweiterungsverwaltung ausschalten lässt.
