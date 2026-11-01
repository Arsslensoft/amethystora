/* The audio module's colours (modules/audio.js): the presets' buttons in GNOME's output menu, drawn as quick settings'
   output buttons are, and the curve. The menu is a submenu of quick settings, outside its box, so the rules name the
   module's own classes. */

#uiGroup .amethystora-audio-presets-row {
  background-color: rgba({{ foreground_rgb }}, 0.06);
  border-color: rgba({{ foreground_rgb }}, 0.10);
}

#uiGroup .amethystora-audio-preset {
  background-color: transparent;
  color: {{ foreground }};
}

#uiGroup .amethystora-audio-preset:hover {
  background-color: rgba({{ foreground_rgb }}, 0.10);
}

#uiGroup .amethystora-audio-preset:checked {
  background-color: {{ foreground }};
  color: {{ background }};
}

#uiGroup .amethystora-audio-curve {
  color: {{ accent_text }};
  -amethystora-audio-grid: rgba({{ foreground_rgb }}, 0.14);
}

#uiGroup .amethystora-audio-tick,
#uiGroup .amethystora-audio-presets-note,
#uiGroup .amethystora-audio-switch-text {
  color: rgba({{ foreground_rgb }}, {{ dim_alpha }});
}
