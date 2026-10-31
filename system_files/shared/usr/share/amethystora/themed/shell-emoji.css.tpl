/* The emoji and symbols picker (Super+.) in the colours of {{ theme_name }}: the theme's text, its accent on the cell
   the keyboard is on, the chosen tab in the text colour with the ground's colour on it, where the module's stylesheet
   (stylesheets/emoji.css) stands in, dark with white text, until a theme has been applied. Every selector starts from
   the shell's top group, one id more than that stylesheet has, which is loaded after this one. */

#uiGroup .amethystora-emoji-picker,
#uiGroup .amethystora-emoji-tones {
  background-color: rgba({{ popover_bg_rgb }}, 0.94);
  border-color: rgba({{ foreground_rgb }}, 0.16);
  color: {{ foreground }};
}

#uiGroup .amethystora-emoji-picker.amethystora-glass-pane,
#uiGroup .amethystora-emoji-tones.amethystora-glass-pane {
  background-color: transparent;
  border-color: transparent;
  -amethystora-glass-edge: rgba({{ foreground_rgb }}, 0.16);
}

#uiGroup.amethystora-glass-solid .amethystora-emoji-picker.amethystora-glass-pane,
#uiGroup.amethystora-glass-solid .amethystora-emoji-tones.amethystora-glass-pane {
  background-color: {{ popover_bg }};
}

#uiGroup .amethystora-emoji-search,
#uiGroup .amethystora-emoji-search:hover,
#uiGroup .amethystora-emoji-search:focus {
  background-color: rgba({{ foreground_rgb }}, 0.08);
  color: {{ foreground }};
  caret-color: {{ accent_text }};
}

#uiGroup .amethystora-emoji-search StLabel.hint-text,
#uiGroup .amethystora-emoji-tab,
#uiGroup .amethystora-emoji-footer {
  color: rgba({{ foreground_rgb }}, {{ dim_alpha }});
}

#uiGroup .amethystora-emoji-search-icon {
  color: {{ accent_text }};
}

#uiGroup .amethystora-emoji-tab:hover,
#uiGroup .amethystora-emoji-cell:hover {
  background-color: rgba({{ foreground_rgb }}, 0.12);
}

#uiGroup .amethystora-emoji-tab:focus {
  border-color: {{ accent_text }};
}

#uiGroup .amethystora-emoji-tab:checked {
  background-color: {{ foreground }};
  color: {{ background }};
}

#uiGroup .amethystora-emoji-cell {
  color: {{ foreground }};
}

#uiGroup .amethystora-emoji-cell:focus,
#uiGroup .amethystora-emoji-cell:selected {
  background-color: rgba({{ accent_rgb }}, 0.45);
}

#uiGroup .amethystora-emoji-key {
  background-color: rgba({{ foreground_rgb }}, 0.12);
  color: {{ foreground }};
}
