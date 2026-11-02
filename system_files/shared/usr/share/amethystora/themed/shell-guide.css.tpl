/* Show me, on my screen in the colours of {{ theme_name }} (modules/guide.js): the ring in the accent, the card on the glass's
   panel in the text colour, its one strong button the accent as a fill. */

#uiGroup .amethystora-guide-ring {
  border-color: {{ accent_text }};
  box-shadow: 0 0 18px rgba({{ foreground_rgb }}, 0.35);
}

#uiGroup .amethystora-guide-card {
  background-color: rgba({{ background_rgb }}, 0.96);
  border-color: rgba({{ foreground_rgb }}, 0.16);
  color: {{ foreground }};
}

#uiGroup .amethystora-guide-button {
  background-color: rgba({{ foreground_rgb }}, 0.1);
  color: {{ foreground }};
}

#uiGroup .amethystora-guide-button.primary {
  background-color: {{ accent_bg }};
  color: {{ accent_fg }};
}
