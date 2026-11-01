/* Tools at hand in the colours of {{ theme_name }} (modules/tools.js): the loupe's ring in the text colour, its label and
   hint on the glass's panel; the microphone's mark in the theme's warning colours; What is using this?'s tiles and Quit a
   tint of the text colour, its second lines the text colour at the board's strength. A QR code stays dark on white on
   every theme: that is what a camera reads. */

#uiGroup .amethystora-tools-loupe {
  border-color: {{ foreground }};
}

#uiGroup .amethystora-tools-picker-label,
#uiGroup .amethystora-tools-picker-hint {
  background-color: rgba({{ background_rgb }}, 0.78);
  border-color: rgba({{ foreground_rgb }}, 0.16);
  color: {{ foreground }};
}

#uiGroup .amethystora-tools-picker-key {
  background-color: rgba({{ foreground_rgb }}, 0.14);
}

#panel .amethystora-tools-mic-pill {
  background-color: {{ warning_bg }};
  color: {{ warning_fg }};
}

#uiGroup .amethystora-tools-users-icon,
#uiGroup .amethystora-tools-users-quit {
  background-color: rgba({{ foreground_rgb }}, 0.08);
  color: {{ foreground }};
}

#uiGroup .amethystora-tools-users-quit:hover,
#uiGroup .amethystora-tools-users-quit:focus {
  background-color: rgba({{ foreground_rgb }}, 0.16);
}

#uiGroup .amethystora-tools-users-detail,
#uiGroup .amethystora-tools-users-footer,
#uiGroup .amethystora-tools-users-status,
#uiGroup .amethystora-tools-qr-detail {
  color: rgba({{ foreground_rgb }}, 0.8);
}
