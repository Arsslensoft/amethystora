/* The Uninstall question's list in the colours of {{ theme_name }}: lights over the dialog in the theme's text colour,
   its accent for the chosen mark and the focus ring, its own red as the button's fill, with the label the palette gives it. Every selector starts from the shell's
   top group, one id more than the module's stylesheet (stylesheets/bazaar.css), which stands in until a theme has been
   applied and is loaded after this one; the light variant of that stylesheet is not needed, since the text colour is
   dark on a light theme. */

#uiGroup .amethystora-bazaar-choices {
  background-color: rgba({{ foreground_rgb }}, 0.07);
  box-shadow: 0 2px 4px rgba(0, 0, 0, 0.22);
}

#uiGroup .amethystora-bazaar-choice:hover {
  background-color: rgba({{ foreground_rgb }}, 0.09);
}

#uiGroup .amethystora-bazaar-choice:focus {
  background-color: rgba({{ foreground_rgb }}, 0.05);
  box-shadow: inset 0 0 0 2px {{ accent_bg }};
}

#uiGroup .amethystora-bazaar-choice:active {
  background-color: rgba({{ foreground_rgb }}, 0.17);
}

#uiGroup .amethystora-bazaar-choice-detail {
  color: rgba({{ foreground_rgb }}, {{ dim_alpha }});
}

#uiGroup .amethystora-bazaar-line {
  background-color: rgba({{ foreground_rgb }}, 0.14);
}

#uiGroup .amethystora-bazaar-mark {
  border-color: {{ outline }};
}

#uiGroup .amethystora-bazaar-choice:checked .amethystora-bazaar-mark {
  border-color: {{ accent_bg }};
  background-color: {{ foreground }};
}

#uiGroup .modal-dialog .modal-dialog-button-box .modal-dialog-button.amethystora-bazaar-uninstall {
  color: {{ destructive_fg }};
  background-color: {{ destructive_bg }};
}

#uiGroup .modal-dialog .modal-dialog-button-box .modal-dialog-button.amethystora-bazaar-uninstall:hover,
#uiGroup .modal-dialog .modal-dialog-button-box .modal-dialog-button.amethystora-bazaar-uninstall:focus {
  background-color: rgba({{ destructive_bg_rgb }}, 0.88);
}

#uiGroup .modal-dialog .modal-dialog-button-box .modal-dialog-button.amethystora-bazaar-uninstall:active {
  background-color: rgba({{ destructive_bg_rgb }}, 0.76);
}

#uiGroup .modal-dialog .modal-dialog-button-box .modal-dialog-button.amethystora-bazaar-uninstall:focus {
  box-shadow: inset 0 0 0 2px rgba({{ destructive_fg_rgb }}, 0.6);
}
