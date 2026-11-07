/* The windows module in the colours of {{ theme_name }}: its panes (the snap chooser, the window switcher, the Super
   card, the show-desktop hint) a veil of the theme's background over the glass with a hairline of its text colour,
   the layouts' cells, the place shown first, the selection ring, the matched letters, the caret, the corner glow and
   the ring of the copy kept in view in the accent, and the veils in the background. Every selector starts from the
   shell's top group, one id more than the module's own stylesheet (stylesheets/windows.css), which only stands in
   until a theme has been applied. */

#uiGroup .amethystora-windows-chooser,
#uiGroup .amethystora-windows-switcher,
#uiGroup .amethystora-windows-card,
#uiGroup .amethystora-windows-desktop-hint {
  border-color: rgba({{ foreground_rgb }}, 0.16);
  border-top-color: rgba({{ foreground_rgb }}, 0.45);
  color: {{ foreground }};
}

#uiGroup .amethystora-windows-chooser {
  background-color: rgba({{ background_rgb }}, 0.46);
}

#uiGroup .amethystora-windows-switcher,
#uiGroup .amethystora-windows-card,
#uiGroup .amethystora-windows-desktop-hint {
  background-color: rgba({{ background_rgb }}, 0.55);
}

#uiGroup .amethystora-windows-layout {
  background-color: rgba({{ foreground_rgb }}, 0.08);
}

#uiGroup .amethystora-windows-cell {
  background-color: rgba({{ accent_rgb }}, 0.35);
}

#uiGroup .amethystora-windows-cell:checked {
  background-color: {{ accent }};
}

#uiGroup .amethystora-windows-preview,
#uiGroup .amethystora-windows-switcher-search {
  color: {{ accent }};
}

#uiGroup .amethystora-windows-veil {
  background-color: rgba({{ background_rgb }}, 0.42);
}

#uiGroup .amethystora-windows-card-veil {
  background-color: rgba({{ background_rgb }}, 0.38);
}

#uiGroup .amethystora-windows-switcher-field {
  background-color: rgba({{ foreground_rgb }}, 0.08);
}

#uiGroup .amethystora-windows-switcher-caret {
  background-color: {{ accent }};
}

#uiGroup .amethystora-windows-switcher-hint {
  color: rgba({{ foreground_rgb }}, 0.55);
}

#uiGroup .amethystora-windows-switcher-count,
#uiGroup .amethystora-windows-switcher-empty,
#uiGroup .amethystora-windows-switcher-footer-text,
#uiGroup .amethystora-windows-card-heading,
#uiGroup .amethystora-windows-card-footer-text {
  color: rgba({{ foreground_rgb }}, 0.75);
}

#uiGroup .amethystora-windows-switcher-thumb {
  border-color: rgba({{ foreground_rgb }}, 0.14);
  background-color: rgba({{ background_rgb }}, 0.7);
}

#uiGroup .amethystora-windows-switcher-ring {
  border-color: {{ accent }};
}

#uiGroup .amethystora-windows-switcher-key {
  background-color: rgba({{ foreground_rgb }}, 0.12);
}

#uiGroup .amethystora-windows-key {
  background-color: rgba({{ foreground_rgb }}, 0.14);
}

#uiGroup .amethystora-windows-glow {
  background-gradient-start: rgba({{ accent_rgb }}, 0.6);
  background-gradient-end: rgba({{ accent_rgb }}, 0);
}

#uiGroup .amethystora-windows-keep-ring {
  box-shadow: 0 0 0 2px {{ accent }};
}

#uiGroup .amethystora-windows-ontop {
  border-color: {{ accent }};
}

#uiGroup .amethystora-windows-ontop-glow {
  background-color: {{ accent }};
  box-shadow: 0 0 22px rgba({{ accent_rgb }}, 0.55);
}

#uiGroup .amethystora-windows-ontop-pin {
  background-color: {{ accent }};
  color: {{ accent_fg }};
}

#uiGroup .amethystora-windows-osd {
  border-color: rgba({{ foreground_rgb }}, 0.16);
  background-color: rgba({{ background_rgb }}, 0.66);
  color: {{ foreground }};
}

#uiGroup .amethystora-windows-frame {
  -amethystora-frame-start: {{ color3 }};
  -amethystora-frame-middle: {{ accent }};
  -amethystora-frame-end: {{ color6 }};
}

/* The strip under the terminal's lights: kitty's title bar, which kitty-theme.conf.tpl gives the theme's background */
#uiGroup .amethystora-windows-lights {
  background-color: {{ background }};
}
