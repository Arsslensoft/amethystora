/* The live pill in the colours of {{ theme_name }}: its plate in the background's tint with a hairline of the text
   colour, the link's and quiet's plates in the accent, the battery's question in the palette's warning colour and
   power saver's word in its success colour; the meter and rings in the accent's text colour; the privacy dots in the
   palette's status colours (microphone warning, camera success, screen error, location accent); and the glow round
   the screen in its error colour. The selectors start from the shell's top group, which the module's own stylesheet
   does not, so these win over its stand-ins. */

#uiGroup .amethystora-pill-piece {
  background-color: rgba({{ background_rgb }}, 0.62);
  border-color: rgba({{ foreground_rgb }}, 0.14);
}
#uiGroup .amethystora-pill-plate-link .amethystora-pill-piece { background-color: rgba({{ accent_bg_rgb }}, 0.5); }
#uiGroup .amethystora-pill-plate-quiet .amethystora-pill-piece { background-color: rgba({{ accent_bg_rgb }}, 0.55); }
#uiGroup .amethystora-pill-plate-battery .amethystora-pill-piece {
  background-color: {{ warning_bg }};
  border-color: {{ warning_bg }};
}
#uiGroup .amethystora-pill-plate-flat .amethystora-pill-piece {
  background-color: {{ error_bg }};
  border-color: {{ error_bg }};
}
#uiGroup .amethystora-pill-plate-saver .amethystora-pill-piece { background-color: rgba({{ success_bg_rgb }}, 0.22); }

/* Two at once, the ring round the plate, and the glow when something ends */
#uiGroup .amethystora-pill-bubble {
  background-color: rgba({{ background_rgb }}, 0.62);
  border-color: rgba({{ foreground_rgb }}, 0.14);
}
#uiGroup .amethystora-pill-bubble-box { color: {{ foreground }}; }
#uiGroup .amethystora-pill-plate-hello .amethystora-pill-piece { background-color: rgba({{ accent_bg_rgb }}, 0.55); }
#uiGroup .amethystora-pill-plate-code .amethystora-pill-piece { background-color: rgba({{ accent_bg_rgb }}, 0.55); }
#uiGroup .amethystora-pill-drop {
  background-color: rgba({{ background_rgb }}, 0.82);
  border-color: rgba({{ foreground_rgb }}, 0.14);
}
#uiGroup .amethystora-pill-drop-tile {
  color: {{ foreground }};
  background-color: rgba({{ foreground_rgb }}, 0.08);
}
#uiGroup .amethystora-pill-drop-tile:hover {
  background-color: {{ accent_bg }};
  color: {{ accent_fg }};
}
#uiGroup .amethystora-pill-moving-awake { color: rgba({{ foreground_rgb }}, 0.8); }
#uiGroup .amethystora-pill-card .amethystora-pill-scrub {
  color: {{ foreground }};
  -barlevel-background-color: rgba({{ foreground_rgb }}, 0.2);
  -barlevel-active-background-color: {{ foreground }};
}
#uiGroup .amethystora-pill-card .amethystora-pill-moving-name { color: {{ foreground }}; }
#uiGroup .amethystora-pill-round-ring {
  color: {{ accent_text }};
  -amethystora-pill-track: rgba({{ foreground_rgb }}, 0.14);
}
#uiGroup .amethystora-pill-flash {
  border-color: {{ accent_text }};
  box-shadow: 0 0 14px 2px rgba({{ accent_bg_rgb }}, 0.9);
}

#uiGroup .amethystora-pill-content { color: {{ foreground }}; }
#uiGroup .amethystora-pill-battery { color: {{ warning_fg }}; }
#uiGroup .amethystora-pill-flat { color: {{ error_fg }}; }
#uiGroup .amethystora-pill-bar { background-color: {{ accent_text }}; }
#uiGroup .amethystora-pill-ring {
  color: {{ accent_text }};
  -amethystora-pill-track: rgba({{ foreground_rgb }}, 0.2);
}
#uiGroup .amethystora-pill-ring-large {
  color: {{ accent_text }};
  -amethystora-pill-track: rgba({{ foreground_rgb }}, 0.15);
}
#uiGroup .amethystora-pill-call-icon { color: {{ success_text }}; }
#uiGroup .amethystora-pill-rec-dot { background-color: {{ error_bg }}; }

#uiGroup .amethystora-pill-chip {
  background-color: rgba({{ foreground_rgb }}, 0.14);
  color: {{ foreground }};
}
#uiGroup .amethystora-pill-chip.amethystora-pill-small-button {
  background-color: rgba({{ foreground_rgb }}, 0.08);
  border-color: rgba({{ foreground_rgb }}, 0.14);
}
#uiGroup .amethystora-pill-chip.amethystora-pill-small-button-primary {
  background-color: {{ accent_bg }};
  color: {{ accent_fg }};
}
#uiGroup .amethystora-pill-chip.amethystora-pill-battery-button {
  background-color: rgba({{ background_rgb }}, 0.9);
  color: {{ foreground }};
}

/* The card */
#uiGroup .amethystora-pill-card .amethystora-pill-button {
  background-color: rgba({{ foreground_rgb }}, 0.1);
  color: {{ foreground }};
}
#uiGroup .amethystora-pill-card .amethystora-pill-button:hover,
#uiGroup .amethystora-pill-card .amethystora-pill-button:focus { background-color: rgba({{ foreground_rgb }}, 0.16); }
#uiGroup .amethystora-pill-card .amethystora-pill-button-primary,
#uiGroup .amethystora-pill-card .amethystora-pill-button-primary:hover,
#uiGroup .amethystora-pill-card .amethystora-pill-button-primary:focus {
  background-color: {{ accent_bg }};
  color: {{ accent_fg }};
}
#uiGroup .amethystora-pill-card .amethystora-pill-button:insensitive { color: rgba({{ foreground_rgb }}, 0.4); }
#uiGroup .amethystora-pill-card .amethystora-pill-card-label,
#uiGroup .amethystora-pill-card .amethystora-pill-card-title,
#uiGroup .amethystora-pill-card .amethystora-pill-timer-big { color: {{ foreground }}; }
#uiGroup .amethystora-pill-card .amethystora-pill-card-text,
#uiGroup .amethystora-pill-card .amethystora-pill-card-small,
#uiGroup .amethystora-pill-card .amethystora-pill-card-clock { color: rgba({{ foreground_rgb }}, 0.8); }
#uiGroup .amethystora-pill-card .amethystora-pill-card-clock:hover,
#uiGroup .amethystora-pill-card .amethystora-pill-card-clock:focus,
#uiGroup .amethystora-pill-card .amethystora-pill-card-row:hover,
#uiGroup .amethystora-pill-card .amethystora-pill-card-row:focus { background-color: rgba({{ foreground_rgb }}, 0.08); }
#uiGroup .amethystora-pill-card .amethystora-pill-card-row { color: {{ foreground }}; }
#uiGroup .amethystora-pill-card .amethystora-pill-card-line { background-color: rgba({{ foreground_rgb }}, 0.12); }
#uiGroup .amethystora-pill-card .amethystora-pill-progress { background-color: rgba({{ foreground_rgb }}, 0.15); }
#uiGroup .amethystora-pill-card .amethystora-pill-progress-fill { background-color: {{ accent_text }}; }

/* The quick timer in the calendar */
#uiGroup .amethystora-pill-timer-title { color: {{ foreground }}; }
#uiGroup .amethystora-pill-timer-button {
  background-color: rgba({{ foreground_rgb }}, 0.1);
  color: {{ foreground }};
}
#uiGroup .amethystora-pill-timer-button:hover,
#uiGroup .amethystora-pill-timer-button:focus { background-color: rgba({{ foreground_rgb }}, 0.16); }
#uiGroup .amethystora-pill-timer-button-primary,
#uiGroup .amethystora-pill-timer-button-primary:hover,
#uiGroup .amethystora-pill-timer-button-primary:focus {
  background-color: {{ accent_bg }};
  color: {{ accent_fg }};
}

/* The privacy dots and their list */
#uiGroup .amethystora-pill-dots { background-color: rgba({{ background_rgb }}, 0.45); }
#uiGroup .amethystora-pill-dot-microphone { background-color: {{ warning_bg }}; }
#uiGroup .amethystora-pill-dot-camera { background-color: {{ success_bg }}; }
#uiGroup .amethystora-pill-dot-screen { background-color: {{ error_bg }}; }
#uiGroup .amethystora-pill-dot-location { background-color: {{ accent_bg }}; }
#uiGroup .amethystora-pill-privacy-row { color: {{ foreground }}; }
#uiGroup .amethystora-pill-privacy-row-off { color: rgba({{ foreground_rgb }}, 0.7); }
#uiGroup .amethystora-pill-privacy-who { color: rgba({{ foreground_rgb }}, 0.8); }
#uiGroup .amethystora-pill-mark-microphone { color: {{ warning_bg }}; border-color: {{ warning_bg }}; }
#uiGroup .amethystora-pill-mark-camera { color: {{ success_bg }}; border-color: {{ success_bg }}; }
#uiGroup .amethystora-pill-mark-screen { color: {{ error_bg }}; border-color: {{ error_bg }}; }
#uiGroup .amethystora-pill-mark-location { color: {{ accent_bg }}; border-color: {{ accent_bg }}; }
#uiGroup .amethystora-pill-privacy-list .amethystora-pill-small-button {
  background-color: rgba({{ foreground_rgb }}, 0.1);
  color: {{ foreground }};
}

/* The battery's time left, and the digest */
#uiGroup .amethystora-pill-tip-first,
#uiGroup .amethystora-pill-digest-title,
#uiGroup .amethystora-pill-digest-row { color: {{ foreground }}; }
#uiGroup .amethystora-pill-tip-second,
#uiGroup .amethystora-pill-digest-count,
#uiGroup .amethystora-pill-digest-more { color: rgba({{ foreground_rgb }}, 0.8); }
#uiGroup .amethystora-pill-digest-button {
  background-color: rgba({{ foreground_rgb }}, 0.12);
  color: {{ foreground }};
}
#uiGroup .amethystora-pill-digest-button-primary {
  background-color: {{ accent_bg }};
  color: {{ accent_fg }};
}

/* Volume and brightness */
#uiGroup .amethystora-glass-osd.amethystora-pill-osd .level,
#uiGroup .amethystora-pill-osd .level {
  -barlevel-background-color: rgba({{ foreground_rgb }}, 0.18);
  -barlevel-active-background-color: {{ accent_text }};
}

/* The glow round a screen that is recorded or shared: a line at the edge and a glow fading inwards */
#uiGroup .amethystora-pill-glow-top {
  border-top: 2px solid rgba({{ error_bg_rgb }}, 0.95);
  background-gradient-direction: vertical;
  background-gradient-start: rgba({{ error_bg_rgb }}, 0.45);
  background-gradient-end: rgba({{ error_bg_rgb }}, 0);
}
#uiGroup .amethystora-pill-glow-bottom {
  border-bottom: 2px solid rgba({{ error_bg_rgb }}, 0.95);
  background-gradient-direction: vertical;
  background-gradient-start: rgba({{ error_bg_rgb }}, 0);
  background-gradient-end: rgba({{ error_bg_rgb }}, 0.45);
}
#uiGroup .amethystora-pill-glow-left {
  border-left: 2px solid rgba({{ error_bg_rgb }}, 0.95);
  background-gradient-direction: horizontal;
  background-gradient-start: rgba({{ error_bg_rgb }}, 0.45);
  background-gradient-end: rgba({{ error_bg_rgb }}, 0);
}
#uiGroup .amethystora-pill-glow-right {
  border-right: 2px solid rgba({{ error_bg_rgb }}, 0.95);
  background-gradient-direction: horizontal;
  background-gradient-start: rgba({{ error_bg_rgb }}, 0);
  background-gradient-end: rgba({{ error_bg_rgb }}, 0.45);
}

/* Later and mute: the banner's buttons as the board's, Later held open filled, and the groups in the calendar's list */
#uiGroup .notification-button.amethystora-pill-later-button {
  background-color: rgba({{ foreground_rgb }}, 0.08);
  border: 1px solid rgba({{ foreground_rgb }}, 0.14);
  color: {{ foreground }};
}
#uiGroup .notification-button.amethystora-pill-later-button:hover,
#uiGroup .notification-button.amethystora-pill-later-button:focus { background-color: rgba({{ foreground_rgb }}, 0.14); }
#uiGroup .notification-button.amethystora-pill-later-button:checked {
  background-color: {{ foreground }};
  border-color: {{ foreground }};
  color: {{ background }};
}
#uiGroup .amethystora-pill-later-heading { color: {{ accent_text }}; }
#uiGroup .amethystora-pill-later-row {
  background-color: rgba({{ foreground_rgb }}, 0.06);
  color: {{ foreground }};
}
#uiGroup .amethystora-pill-later-text { color: rgba({{ foreground_rgb }}, 0.8); }
#uiGroup .amethystora-pill-later-link { color: {{ accent_text }}; }
#uiGroup .amethystora-pill-later-link:hover,
#uiGroup .amethystora-pill-later-link:focus { background-color: rgba({{ foreground_rgb }}, 0.08); }

/* What's new: the plate in the accent, as the board's, and the card's rows */
#uiGroup .amethystora-pill-plate-news .amethystora-pill-piece { background-color: rgba({{ accent_bg_rgb }}, 0.6); }
#uiGroup .amethystora-pill-card .amethystora-pill-news-title,
#uiGroup .amethystora-pill-card .amethystora-pill-news-item { color: {{ foreground }}; }
#uiGroup .amethystora-pill-card .amethystora-pill-news-versions,
#uiGroup .amethystora-pill-card .amethystora-pill-news-once { color: rgba({{ foreground_rgb }}, 0.8); }
#uiGroup .amethystora-pill-card .amethystora-pill-news-area { color: rgba({{ foreground_rgb }}, 0.85); }
#uiGroup .amethystora-pill-card .amethystora-pill-news-row { background-color: rgba({{ foreground_rgb }}, 0.06); }
#uiGroup .amethystora-pill-card .amethystora-pill-news-icon {
  background-color: rgba({{ accent_bg_rgb }}, 0.45);
  color: {{ accent_fg }};
}
