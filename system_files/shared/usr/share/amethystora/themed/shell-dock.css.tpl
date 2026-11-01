/* The dock in the colours of {{ theme_name }}: the veil on a light theme and the wash on a tile in the text colour (dark
   on a light theme, light on a dark one), the dots and the app grid's in the text colour, the bar of progress from the accent's fill to its
   text colour, and the count's badge in the palette's red with the background's colour on it (the count and the bar on
   the apps' icons in the overview too); then the marks, the cards and the drops of the dock's newer parts.
   The selectors start from the shell's top group, one id more than the dock's own stylesheet has for them, which only stands in until a theme has been applied; the veil's does
   not, so that the glass's colours for the dock in the overview stay above it. */

#dash.amethystora-dock-dash.amethystora-dock-light .dash-background {
  background-color: rgba({{ foreground_rgb }}, 0.22);
  border-color: rgba({{ background_rgb }}, 0.05);
}

#uiGroup #dash.amethystora-dock-dash .dash-item-container .overview-tile:hover .overview-icon,
#uiGroup #dash.amethystora-dock-dash .dash-item-container .overview-tile:focus .overview-icon,
#uiGroup #dash.amethystora-dock-dash .dash-item-container .show-apps:hover .overview-icon,
#uiGroup #dash.amethystora-dock-dash .dash-item-container .show-apps:focus .overview-icon,
#uiGroup #dash.amethystora-dock-dash .dash-item-container .amethystora-dock-focused .overview-icon {
  background-color: rgba({{ foreground_rgb }}, 0.2);
}

#uiGroup #dash.amethystora-dock-dash .dash-item-container .show-apps .overview-icon {
  color: {{ foreground }};
}

#uiGroup #dash.amethystora-dock-dash .dash-separator {
  background-color: rgba({{ foreground_rgb }}, 0.1);
}

#uiGroup #dash.amethystora-dock-dash .amethystora-dock-dot {
  background-color: {{ foreground }};
}

#uiGroup .amethystora-dock-badge {
  background-color: {{ color1 }};
  color: {{ background }};
}

/* What arrived in the Downloads stack (R5-35): the theme's yellow, not an alarm's red */
#uiGroup .amethystora-dock-badge.amethystora-dock-arrivals {
  background-color: {{ color3 }};
  color: {{ background }};
}

#uiGroup .amethystora-dock-progress {
  background-color: rgba({{ background_rgb }}, 0.75);
}

#uiGroup .amethystora-dock-progress-fill {
  background-gradient-start: {{ accent_bg }};
  background-gradient-end: {{ accent_text }};
}

/* What marks a running app, and the numbers Alt+N opens */
#uiGroup .amethystora-dock-bar {
  background-color: {{ accent_text }};
}

#uiGroup .amethystora-dock-glow {
  background-gradient-start: rgba({{ accent_text_rgb }}, 0.95);
  background-gradient-end: rgba({{ accent_text_rgb }}, 0);
}

#uiGroup .amethystora-dock-number {
  background-color: {{ foreground }};
  color: {{ background }};
}

/* The ring while an app starts: its colour, and its leading end's */
#uiGroup .amethystora-dock-halo {
  color: {{ accent_text }};
}

#uiGroup .amethystora-dock-halo-tip {
  color: {{ foreground }};
}

#uiGroup .amethystora-dock-recent-caption {
  color: {{ dim_text }};
}

/* Previews, the fan and its veil */
#uiGroup .amethystora-dock-preview,
#uiGroup .amethystora-dock-fan-tile {
  background-color: rgba({{ background_rgb }}, 0.75);
  border-color: rgba({{ foreground_rgb }}, 0.14);
  color: {{ foreground }};
}

#uiGroup .amethystora-dock-preview:hover,
#uiGroup .amethystora-dock-preview:focus {
  border-color: rgba({{ accent_text_rgb }}, 0.6);
}

#uiGroup .amethystora-dock-preview-title,
#uiGroup .amethystora-dock-fan-title {
  background-color: rgba({{ foreground_rgb }}, 0.06);
}

#uiGroup .amethystora-dock-fan-tile.amethystora-dock-fan-ringed {
  box-shadow: 0 0 0 2px {{ accent_text }};
}

#uiGroup .amethystora-dock-veil {
  background-color: rgba({{ background_rgb }}, 0.35);
}

/* Folders: the picture's ground, the item about to take another, the card */
#uiGroup .amethystora-dock-folder-icon {
  background-color: rgba({{ foreground_rgb }}, 0.14);
}

#uiGroup .amethystora-dock-folder-target .overview-icon {
  box-shadow: 0 0 0 2px {{ accent_bg }};
}

#uiGroup .amethystora-dock-folder,
#uiGroup .amethystora-dock-stack,
#uiGroup .amethystora-dock-phone-card {
  color: {{ foreground }};
}

#uiGroup .amethystora-dock-folder-member:hover,
#uiGroup .amethystora-dock-folder-member:focus,
#uiGroup .amethystora-dock-stack-entry:hover,
#uiGroup .amethystora-dock-stack-entry:focus {
  background-color: rgba({{ foreground_rgb }}, 0.10);
}

#uiGroup .amethystora-dock-folder-name {
  color: {{ accent_text }};
}

#uiGroup .amethystora-dock-folder-name:hover,
#uiGroup .amethystora-dock-folder-name:focus {
  background-color: rgba({{ foreground_rgb }}, 0.10);
}

/* Stacks */
#uiGroup .amethystora-dock-stack-chip,
#uiGroup .amethystora-dock-stack-open {
  background-color: rgba({{ foreground_rgb }}, 0.10);
}

#uiGroup .amethystora-dock-stack-chip,
#uiGroup .amethystora-dock-stack-empty {
  color: {{ dim_text }};
}

#uiGroup .amethystora-dock-stack-open:hover,
#uiGroup .amethystora-dock-stack-open:focus {
  background-color: rgba({{ foreground_rgb }}, 0.18);
}

/* Drops: the count on a stack, the phone's progress, the pill keeping folders */
#uiGroup .amethystora-dock-drop-badge {
  background-color: {{ success_bg }};
  color: {{ success_fg }};
}

#uiGroup .amethystora-dock-phone-track {
  background-color: rgba({{ foreground_rgb }}, 0.12);
}

#uiGroup .amethystora-dock-phone-fill {
  background-gradient-start: {{ accent_bg }};
  background-gradient-end: {{ accent_text }};
}

#uiGroup #dash.amethystora-dock-dash .dash-background.amethystora-dock-pill-drop {
  box-shadow: 0 0 0 2px {{ accent_text }};
}

/* The specks of a dissolve, and the hidden dock's bar */
#uiGroup .amethystora-dock-speck {
  background-gradient-direction: radial;
  background-gradient-start: {{ foreground }};
  background-gradient-end: {{ accent_text }};
  box-shadow: 0 0 8px rgba({{ accent_text_rgb }}, 0.7);
}

#uiGroup .amethystora-dock-hint {
  background-color: rgba({{ accent_text_rgb }}, 0.55);
}
