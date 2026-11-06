/* The agent on the desktop in the colours of {{ theme_name }} (modules/agent.js): the selected text on a darker well,
   the question's field a tint of the text colour, Ask filled with the text colour and its word in the background's,
   the dimmed words at the board's strength; in the live pill, the agent at work on the accent's plate, waiting for the
   person on the warning colour's, finished on a fainter accent, each change's row a tint of the text colour, a row that
   waits a tint of the warning colour, and Undo, Show and History in the accent's text colour. The selectors start from
   the shell's top group, so these win over the module's own stand-ins. */

#uiGroup .amethystora-agent-ask-title { color: {{ foreground }}; }
#uiGroup .amethystora-agent-ask-quote {
  background-color: rgba({{ background_rgb }}, 0.5);
  color: rgba({{ foreground_rgb }}, 0.85);
}
#uiGroup .amethystora-agent-ask-field {
  background-color: rgba({{ foreground_rgb }}, 0.08);
  border-color: rgba({{ foreground_rgb }}, 0.16);
  color: {{ foreground }};
  caret-color: {{ foreground }};
  selection-background-color: {{ accent_bg }};
  selected-color: {{ accent_fg }};
}
#uiGroup .amethystora-agent-ask-field StLabel.hint-text { color: rgba({{ foreground_rgb }}, 0.55); }
#uiGroup .amethystora-agent-ask-hint { color: rgba({{ foreground_rgb }}, 0.8); }
#uiGroup .modal-dialog .modal-dialog-button-box .modal-dialog-button.amethystora-agent-ask-button {
  background-color: rgba({{ foreground_rgb }}, 0.08);
  border-color: rgba({{ foreground_rgb }}, 0.14);
  color: {{ foreground }};
}
#uiGroup .modal-dialog .modal-dialog-button-box .modal-dialog-button.amethystora-agent-ask-button:hover,
#uiGroup .modal-dialog .modal-dialog-button-box .modal-dialog-button.amethystora-agent-ask-button:focus {
  background-color: rgba({{ foreground_rgb }}, 0.16);
}
#uiGroup .modal-dialog .modal-dialog-button-box .modal-dialog-button.amethystora-agent-ask-primary,
#uiGroup .modal-dialog .modal-dialog-button-box .modal-dialog-button.amethystora-agent-ask-primary:hover,
#uiGroup .modal-dialog .modal-dialog-button-box .modal-dialog-button.amethystora-agent-ask-primary:focus {
  background-color: {{ foreground }};
  border-color: {{ foreground }};
  color: {{ background }};
}

#uiGroup .amethystora-pill-plate-agent .amethystora-pill-piece { background-color: rgba({{ accent_bg_rgb }}, 0.6); }
#uiGroup .amethystora-pill-plate-agent-ask .amethystora-pill-piece { background-color: rgba({{ warning_bg_rgb }}, 0.45); }
#uiGroup .amethystora-pill-plate-agent-done .amethystora-pill-piece { background-color: rgba({{ accent_bg_rgb }}, 0.35); }
#uiGroup .amethystora-pill-card .amethystora-agent-card-title,
#uiGroup .amethystora-pill-card .amethystora-agent-act { color: {{ foreground }}; }
#uiGroup .amethystora-pill-card .amethystora-agent-card-who,
#uiGroup .amethystora-pill-card .amethystora-agent-act-how,
#uiGroup .amethystora-pill-card .amethystora-agent-act-state { color: rgba({{ foreground_rgb }}, 0.8); }
#uiGroup .amethystora-pill-card .amethystora-agent-act { background-color: rgba({{ foreground_rgb }}, 0.06); }
#uiGroup .amethystora-pill-card .amethystora-agent-act-ask { background-color: rgba({{ warning_bg_rgb }}, 0.18); }
#uiGroup .amethystora-pill-card .amethystora-agent-link { color: {{ accent_text }}; }
#uiGroup .amethystora-pill-card .amethystora-agent-link:hover,
#uiGroup .amethystora-pill-card .amethystora-agent-link:focus { background-color: rgba({{ foreground_rgb }}, 0.08); }
#uiGroup .amethystora-pill-card .amethystora-agent-card .amethystora-pill-button {
  background-color: rgba({{ foreground_rgb }}, 0.08);
  border-color: rgba({{ foreground_rgb }}, 0.14);
}

/* Ask right there and Yes or no (modules/agent/ask.js): the field and the exact command on a darker well, the answer on the
   card's own tint */
#uiGroup .amethystora-pill-card .amethystora-agent-field {
  background-color: rgba({{ foreground_rgb }}, 0.08);
  border-color: rgba({{ foreground_rgb }}, 0.16);
  color: {{ foreground }};
  caret-color: {{ foreground }};
  selection-background-color: {{ accent_bg }};
  selected-color: {{ accent_fg }};
}
#uiGroup .amethystora-pill-card .amethystora-agent-field StLabel.hint-text { color: rgba({{ foreground_rgb }}, 0.55); }
#uiGroup .amethystora-pill-card .amethystora-agent-command {
  background-color: rgba({{ background_rgb }}, 0.5);
  color: rgba({{ foreground_rgb }}, 0.9);
}
#uiGroup .amethystora-pill-card .amethystora-agent-answer {
  background-color: rgba({{ foreground_rgb }}, 0.06);
  color: {{ foreground }};
}
#uiGroup .amethystora-pill-card .amethystora-agent-ask-part .amethystora-agent-card-title,
#uiGroup .amethystora-pill-card .amethystora-agent-question .amethystora-agent-card-title { color: {{ foreground }}; }
#uiGroup .amethystora-pill-card .amethystora-agent-ask-part .amethystora-pill-button:not(.amethystora-pill-button-primary),
#uiGroup .amethystora-pill-card .amethystora-agent-question .amethystora-pill-button:not(.amethystora-pill-button-primary) {
  background-color: rgba({{ foreground_rgb }}, 0.08);
  border-color: rgba({{ foreground_rgb }}, 0.14);
}
