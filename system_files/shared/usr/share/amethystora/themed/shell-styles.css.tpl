/* The desktop styles in the colours of {{ theme_name }}: the shape the shell is drawn in, as the theme (style.theme) or
   the account (ame desktop style) chooses it. Amethystora UI's style module gives the shell's top group the class
   amethystora-style-<style>, and every rule here starts from it, so all seven are loaded and only one applies. The
   glass takes each pane's corners from the border-radius given here (modules/glass/effect.js), so the panes always
   match. Each block covers the top bar, the menus and quick settings, the notifications, the dialogs and prompts,
   the on-screen alerts and the window switcher, the buttons and controls, the overview's tiles and the dock; the
   applications' half of each style is style-<style>.gtk.css. Floating is the desktop as it was before there were
   styles, so its block holds only its bar. */

/* ---- Floating: one rounded bar held off the screen's edge ---------------------------------------------------- */

#uiGroup.amethystora-style-floating #panel {
  margin: 8px 10px 0 10px;
  border-radius: 13px;
  border: 1px solid rgba({{ accent_rgb }}, 0.35);
}

#uiGroup.amethystora-style-floating #panel.amethystora-glass-clear {
  background-color: transparent;
  border-color: transparent;
  -amethystora-glass-edge: rgba({{ accent_rgb }}, 0.35);
}

#uiGroup.amethystora-style-floating #panel:overview {
  border-color: transparent;
}

/* ---- Islands: a floating pill for each button, the workspaces as dots ----------------------------------------- */

#uiGroup.amethystora-style-islands #panel,
#uiGroup.amethystora-style-islands #panel.amethystora-glass-clear {
  background-color: transparent;
  margin: 6px 8px 0 8px;
}

#uiGroup.amethystora-style-islands #panel .panel-button {
  margin: 0 3px;
  border-width: 1px;
  border-color: rgba({{ foreground_rgb }}, 0.08);
  border-radius: 14px;
  background-color: rgba({{ background_rgb }}, 0.55);
  color: {{ foreground }};
  -natural-hpadding: 14px;
}

#uiGroup.amethystora-style-islands #panel:overview .panel-button {
  background-color: rgba(0, 0, 0, 0.3);
  color: #ffffff;
}

#uiGroup.amethystora-style-islands .amethystora-workspaces-label,
#uiGroup.amethystora-style-islands .amethystora-workspaces-label.amethystora-workspaces-busy,
#uiGroup.amethystora-style-islands .amethystora-workspaces-label.amethystora-workspaces-empty {
  color: transparent;
  width: 8px;
  height: 8px;
  padding: 0;
  margin: 0 3px;
  border-radius: 4px;
  background-color: rgba({{ foreground_rgb }}, 0.25);
}

#uiGroup.amethystora-style-islands .amethystora-workspaces-label.amethystora-workspaces-busy {
  background-color: rgba({{ foreground_rgb }}, 0.7);
}

#uiGroup.amethystora-style-islands .amethystora-workspaces-label.amethystora-workspaces-active {
  color: transparent;
  width: 22px;
  background-color: {{ accent }};
}

#uiGroup.amethystora-style-islands .amethystora-glass-surface.amethystora-glass-menu { border-radius: 18px; }
#uiGroup.amethystora-style-islands .amethystora-glass-surface.amethystora-glass-panel { border-radius: 28px; }
#uiGroup.amethystora-style-islands .amethystora-glass-surface.amethystora-glass-inner { border-radius: 22px; }
#uiGroup.amethystora-style-islands .amethystora-glass-surface.amethystora-glass-banner { border-radius: 22px; }
#uiGroup.amethystora-style-islands .amethystora-glass-surface.amethystora-glass-osd { border-radius: 26px; }
#uiGroup.amethystora-style-islands .amethystora-glass-surface.amethystora-glass-dialog { border-radius: 24px; }
#uiGroup.amethystora-style-islands .popup-menu-item { border-radius: 10px; }
#uiGroup.amethystora-style-islands .message { border-radius: 18px; }

#uiGroup.amethystora-style-islands .button,
#uiGroup.amethystora-style-islands .modal-dialog-linked-button,
#uiGroup.amethystora-style-islands .notification-button,
#uiGroup.amethystora-style-islands StEntry {
  border-radius: 12px;
}

/* ---- Hairline: crisp corners, thin lines, a solid strip of a bar ---------------------------------------------- */

#uiGroup.amethystora-style-hairline #panel,
#uiGroup.amethystora-style-hairline #panel.amethystora-glass-clear {
  background-color: {{ background }};
  border-bottom: 1px solid rgba({{ foreground_rgb }}, 0.1);
}

#uiGroup.amethystora-style-hairline #panel .panel-button {
  border-width: 0;
  border-radius: 4px;
  color: {{ foreground }};
}

#uiGroup.amethystora-style-hairline #panel:overview,
#uiGroup.amethystora-style-hairline #panel:overview.amethystora-glass-clear {
  background-color: transparent;
  border-color: transparent;
}

#uiGroup.amethystora-style-hairline #panel:overview .panel-button {
  color: #ffffff;
}

#uiGroup.amethystora-style-hairline .amethystora-workspaces-label,
#uiGroup.amethystora-style-hairline .amethystora-workspaces-label.amethystora-workspaces-active {
  border-radius: 0;
  background-color: transparent;
  border-bottom: 2px solid transparent;
}

#uiGroup.amethystora-style-hairline .amethystora-workspaces-label.amethystora-workspaces-active {
  color: {{ accent_text }};
  border-color: {{ accent }};
}

#uiGroup.amethystora-style-hairline .amethystora-glass-surface {
  border: 1px solid rgba({{ foreground_rgb }}, 0.14);
}

#uiGroup.amethystora-style-hairline .amethystora-glass-surface.amethystora-glass-menu { border-radius: 6px; }
#uiGroup.amethystora-style-hairline .amethystora-glass-surface.amethystora-glass-panel { border-radius: 10px; }
#uiGroup.amethystora-style-hairline .amethystora-glass-surface.amethystora-glass-inner { border-radius: 8px; }
#uiGroup.amethystora-style-hairline .amethystora-glass-surface.amethystora-glass-banner { border-radius: 8px; }
#uiGroup.amethystora-style-hairline .amethystora-glass-surface.amethystora-glass-osd { border-radius: 10px; }
#uiGroup.amethystora-style-hairline .amethystora-glass-surface.amethystora-glass-dialog { border-radius: 8px; }
#uiGroup.amethystora-style-hairline .amethystora-glass-surface.amethystora-glass-keyboard { border-radius: 6px; }
#uiGroup.amethystora-style-hairline .popup-menu-item,
#uiGroup.amethystora-style-hairline .amethystora-glass-surface .popup-sub-menu .popup-menu-item:last-child { border-radius: 4px; }
#uiGroup.amethystora-style-hairline .message { border-radius: 6px; }

#uiGroup.amethystora-style-hairline .button,
#uiGroup.amethystora-style-hairline .modal-dialog-linked-button,
#uiGroup.amethystora-style-hairline .notification-button,
#uiGroup.amethystora-style-hairline .icon-button,
#uiGroup.amethystora-style-hairline .quick-toggle,
#uiGroup.amethystora-style-hairline .quick-toggle-menu-button,
#uiGroup.amethystora-style-hairline .amethystora-focus-chip,
#uiGroup.amethystora-style-hairline .switcher-list .item-box,
#uiGroup.amethystora-style-hairline .calendar-day,
#uiGroup.amethystora-style-hairline .events-button,
#uiGroup.amethystora-style-hairline .world-clocks-button,
#uiGroup.amethystora-style-hairline .weather-button,
#uiGroup.amethystora-style-hairline .overview-tile,
#uiGroup.amethystora-style-hairline .overview-icon,
#uiGroup.amethystora-style-hairline .search-entry,
#uiGroup.amethystora-style-hairline StEntry {
  border-radius: 4px;
}

#uiGroup.amethystora-style-hairline #dash.amethystora-dock-dash .dash-background { border-radius: 10px; }

/* ---- Capsule: everything a pill, the clock in a dark capsule, the bar's sides bare over the wallpaper --------- */

#uiGroup.amethystora-style-capsule #panel,
#uiGroup.amethystora-style-capsule #panel.amethystora-glass-clear {
  background-color: transparent;
  margin: 6px 14px 0 14px;
}

#uiGroup.amethystora-style-capsule #panel .panel-button {
  border-width: 0;
  color: {{ foreground }};
  text-shadow: 0 1px 6px rgba({{ background_rgb }}, 0.9);
  icon-shadow: 0 1px 6px rgba({{ background_rgb }}, 0.9);
}

#uiGroup.amethystora-style-capsule #panel #panelCenter .panel-button {
  border-radius: 999px;
  background-color: rgba(0, 0, 0, 0.86);
  color: #ffffff;
  text-shadow: none;
  icon-shadow: none;
  -natural-hpadding: 22px;
}

#uiGroup.amethystora-style-capsule #panel:overview .panel-button {
  color: #ffffff;
}

#uiGroup.amethystora-style-capsule .amethystora-glass-surface.amethystora-glass-menu { border-radius: 24px; }
#uiGroup.amethystora-style-capsule .amethystora-glass-surface.amethystora-glass-panel { border-radius: 32px; }
#uiGroup.amethystora-style-capsule .amethystora-glass-surface.amethystora-glass-inner { border-radius: 26px; }
#uiGroup.amethystora-style-capsule .amethystora-glass-surface.amethystora-glass-banner { border-radius: 30px; }
#uiGroup.amethystora-style-capsule .amethystora-glass-surface.amethystora-glass-osd { border-radius: 999px; }
#uiGroup.amethystora-style-capsule .amethystora-glass-surface.amethystora-glass-dialog { border-radius: 30px; }
#uiGroup.amethystora-style-capsule .amethystora-glass-surface.amethystora-glass-keyboard { border-radius: 24px; }
#uiGroup.amethystora-style-capsule .message { border-radius: 26px; }

#uiGroup.amethystora-style-capsule .popup-menu-item,
#uiGroup.amethystora-style-capsule .amethystora-glass-surface .popup-sub-menu .popup-menu-item:last-child,
#uiGroup.amethystora-style-capsule .button,
#uiGroup.amethystora-style-capsule .modal-dialog-linked-button,
#uiGroup.amethystora-style-capsule .notification-button,
#uiGroup.amethystora-style-capsule .switcher-list .item-box,
#uiGroup.amethystora-style-capsule .events-button,
#uiGroup.amethystora-style-capsule .world-clocks-button,
#uiGroup.amethystora-style-capsule .weather-button,
#uiGroup.amethystora-style-capsule StEntry {
  border-radius: 999px;
}

/* ---- Terminal: monospace, square, lined, solid ---------------------------------------------------------------- */

#uiGroup.amethystora-style-terminal {
  font-family: "{{ mono_font }}", monospace;
}

#uiGroup.amethystora-style-terminal #panel,
#uiGroup.amethystora-style-terminal #panel.amethystora-glass-clear {
  background-color: {{ background }};
  border-bottom: 1px solid rgba({{ foreground_rgb }}, 0.18);
  font-weight: normal;
}

#uiGroup.amethystora-style-terminal #panel .panel-button {
  border-width: 0;
  border-radius: 0;
  color: {{ foreground }};
  font-weight: normal;
}

#uiGroup.amethystora-style-terminal #panel:overview,
#uiGroup.amethystora-style-terminal #panel:overview.amethystora-glass-clear {
  background-color: transparent;
  border-color: transparent;
}

#uiGroup.amethystora-style-terminal #panel:overview .panel-button {
  color: #ffffff;
}

#uiGroup.amethystora-style-terminal .amethystora-workspaces-label,
#uiGroup.amethystora-style-terminal .amethystora-workspaces-label.amethystora-workspaces-active {
  border-radius: 0;
  background-color: transparent;
  border: 1px solid transparent;
}

#uiGroup.amethystora-style-terminal .amethystora-workspaces-label.amethystora-workspaces-active {
  color: {{ accent_text }};
  border-color: {{ accent }};
}

#uiGroup.amethystora-style-terminal .amethystora-meters-bar {
  background-color: {{ color2 }};
}

#uiGroup.amethystora-style-terminal .amethystora-glass-surface,
#uiGroup.amethystora-style-terminal .amethystora-glass-surface:hover,
#uiGroup.amethystora-style-terminal .amethystora-glass-surface:active,
#uiGroup.amethystora-style-terminal .amethystora-glass-surface:focus {
  background-color: rgba({{ background_rgb }}, 0.92);
  border: 1px solid rgba({{ foreground_rgb }}, 0.35);
}

#uiGroup.amethystora-style-terminal .amethystora-glass-surface,
#uiGroup.amethystora-style-terminal .popup-menu-item,
#uiGroup.amethystora-style-terminal .amethystora-glass-surface .popup-sub-menu .popup-menu-item:last-child,
#uiGroup.amethystora-style-terminal .popup-sub-menu,
#uiGroup.amethystora-style-terminal .message,
#uiGroup.amethystora-style-terminal .button,
#uiGroup.amethystora-style-terminal .modal-dialog-linked-button,
#uiGroup.amethystora-style-terminal .notification-button,
#uiGroup.amethystora-style-terminal .icon-button,
#uiGroup.amethystora-style-terminal .quick-toggle,
#uiGroup.amethystora-style-terminal .quick-toggle-menu-button,
#uiGroup.amethystora-style-terminal .quick-toggle-has-menu,
#uiGroup.amethystora-style-terminal .amethystora-focus-chip,
#uiGroup.amethystora-style-terminal .switcher-list .item-box,
#uiGroup.amethystora-style-terminal .calendar-day,
#uiGroup.amethystora-style-terminal .events-button,
#uiGroup.amethystora-style-terminal .world-clocks-button,
#uiGroup.amethystora-style-terminal .weather-button,
#uiGroup.amethystora-style-terminal .overview-tile,
#uiGroup.amethystora-style-terminal .overview-icon,
#uiGroup.amethystora-style-terminal .search-entry,
#uiGroup.amethystora-style-terminal .dash-label,
#uiGroup.amethystora-style-terminal .window-caption,
#uiGroup.amethystora-style-terminal StEntry,
#uiGroup.amethystora-style-terminal #dash.amethystora-dock-dash .dash-background {
  border-radius: 0;
}

#uiGroup.amethystora-style-terminal .button,
#uiGroup.amethystora-style-terminal .modal-dialog-linked-button,
#uiGroup.amethystora-style-terminal .notification-button,
#uiGroup.amethystora-style-terminal StEntry {
  border: 1px solid rgba({{ foreground_rgb }}, 0.3);
}

/* ---- Segments: the bar in blocks of the palette's colours ----------------------------------------------------- */

#uiGroup.amethystora-style-segments #panel,
#uiGroup.amethystora-style-segments #panel.amethystora-glass-clear {
  background-color: transparent;
  margin: 6px 10px 0 10px;
}

#uiGroup.amethystora-style-segments #panel .panel-button {
  margin: 0;
  border-width: 0;
  border-radius: 0;
  color: {{ background }};
  -natural-hpadding: 14px;
}

/* Each block is a colour of the palette, moved in lightness only as far as its label reads on it (4.5:1) */
#uiGroup.amethystora-style-segments #panel .panel-button.amethystora-style-colour1 { background-color: {{ segment1_bg }}; color: {{ segment1_fg }}; }
#uiGroup.amethystora-style-segments #panel .panel-button.amethystora-style-colour2 { background-color: {{ segment2_bg }}; color: {{ segment2_fg }}; }
#uiGroup.amethystora-style-segments #panel .panel-button.amethystora-style-colour3 { background-color: {{ segment3_bg }}; color: {{ segment3_fg }}; }
#uiGroup.amethystora-style-segments #panel .panel-button.amethystora-style-colour4 { background-color: {{ segment4_bg }}; color: {{ segment4_fg }}; }
#uiGroup.amethystora-style-segments #panel .panel-button.amethystora-style-colour5 { background-color: {{ segment5_bg }}; color: {{ segment5_fg }}; }
#uiGroup.amethystora-style-segments #panel .panel-button.amethystora-style-colour6 { background-color: {{ segment6_bg }}; color: {{ segment6_fg }}; }

#uiGroup.amethystora-style-segments #panel .panel-button.amethystora-style-first:ltr,
#uiGroup.amethystora-style-segments #panel .panel-button.amethystora-style-last:rtl {
  border-radius: 12px 0 0 12px;
}

#uiGroup.amethystora-style-segments #panel .panel-button.amethystora-style-last:ltr,
#uiGroup.amethystora-style-segments #panel .panel-button.amethystora-style-first:rtl {
  border-radius: 0 12px 12px 0;
}

#uiGroup.amethystora-style-segments #panel .panel-button.amethystora-style-first.amethystora-style-last {
  border-radius: 12px;
}

#uiGroup.amethystora-style-segments .amethystora-workspaces-label,
#uiGroup.amethystora-style-segments .amethystora-workspaces-label.amethystora-workspaces-busy,
#uiGroup.amethystora-style-segments .amethystora-workspaces-label.amethystora-workspaces-empty {
  color: rgba({{ background_rgb }}, 0.75);
  background-color: transparent;
}

#uiGroup.amethystora-style-segments .amethystora-workspaces-label.amethystora-workspaces-active {
  color: {{ foreground }};
  background-color: {{ background }};
}

#uiGroup.amethystora-style-segments .amethystora-meters-bar {
  background-color: {{ background }};
}

#uiGroup.amethystora-style-segments .amethystora-glass-surface.amethystora-glass-menu { border-radius: 12px; }
#uiGroup.amethystora-style-segments .amethystora-glass-surface.amethystora-glass-panel { border-radius: 18px; }
#uiGroup.amethystora-style-segments .amethystora-glass-surface.amethystora-glass-inner { border-radius: 14px; }
#uiGroup.amethystora-style-segments .amethystora-glass-surface.amethystora-glass-banner { border-radius: 14px; }
#uiGroup.amethystora-style-segments .amethystora-glass-surface.amethystora-glass-osd { border-radius: 16px; }
#uiGroup.amethystora-style-segments .amethystora-glass-surface.amethystora-glass-dialog { border-radius: 14px; }
#uiGroup.amethystora-style-segments .amethystora-glass-surface.amethystora-glass-keyboard { border-radius: 12px; }
#uiGroup.amethystora-style-segments .message { border-radius: 12px; }

#uiGroup.amethystora-style-segments .popup-menu-item,
#uiGroup.amethystora-style-segments .amethystora-glass-surface .popup-sub-menu .popup-menu-item:last-child,
#uiGroup.amethystora-style-segments .button,
#uiGroup.amethystora-style-segments .modal-dialog-linked-button,
#uiGroup.amethystora-style-segments .notification-button,
#uiGroup.amethystora-style-segments .quick-toggle,
#uiGroup.amethystora-style-segments .quick-toggle-menu-button,
#uiGroup.amethystora-style-segments .switcher-list .item-box,
#uiGroup.amethystora-style-segments StEntry {
  border-radius: 8px;
}

#uiGroup.amethystora-style-segments .popup-menu-item:hover,
#uiGroup.amethystora-style-segments .popup-menu-item:focus,
#uiGroup.amethystora-style-segments .popup-menu-item:selected {
  background-color: rgba({{ accent_rgb }}, 0.28);
}

#uiGroup.amethystora-style-segments #dash.amethystora-dock-dash .dash-background { border-radius: 18px; }

/* ---- Neon: dark glass outlined and glowing in the accent ------------------------------------------------------ */

#uiGroup.amethystora-style-neon #panel {
  margin: 8px 12px 0 12px;
  border-radius: 10px;
  border: 1px solid {{ accent }};
  box-shadow: 0 0 16px rgba({{ accent_rgb }}, 0.55);
  background-color: rgba({{ background_rgb }}, 0.8);
}

#uiGroup.amethystora-style-neon #panel.amethystora-glass-clear {
  background-color: rgba({{ background_rgb }}, 0.5);
}

#uiGroup.amethystora-style-neon #panel:overview {
  border-color: transparent;
  box-shadow: none;
  background-color: transparent;
}

#uiGroup.amethystora-style-neon #panel .panel-button {
  border-width: 0;
  border-radius: 6px;
  color: {{ foreground }};
}

#uiGroup.amethystora-style-neon #panel .panel-button.clock-display,
#uiGroup.amethystora-style-neon #panel .clock-display .clock {
  color: {{ accent_text }};
  text-shadow: 0 0 8px rgba({{ accent_rgb }}, 0.9);
}

/* On the clear bar the bar's own glow shows through its half fill and tints it in the accent, against which the
   accent's text no longer reads: there the clock glows white-hot in the accent, as a neon tube's core does */
#uiGroup.amethystora-style-neon #panel.amethystora-glass-clear .panel-button.clock-display,
#uiGroup.amethystora-style-neon #panel.amethystora-glass-clear .clock-display .clock {
  color: {{ foreground }};
}

#uiGroup.amethystora-style-neon .amethystora-workspaces-label,
#uiGroup.amethystora-style-neon .amethystora-workspaces-label.amethystora-workspaces-active {
  border-radius: 6px;
  background-color: transparent;
  border: 1px solid transparent;
}

#uiGroup.amethystora-style-neon .amethystora-workspaces-label.amethystora-workspaces-active {
  color: {{ foreground }};
  border-color: {{ accent }};
  box-shadow: 0 0 10px rgba({{ accent_rgb }}, 0.6);
  text-shadow: 0 0 8px {{ accent }};
}

#uiGroup.amethystora-style-neon .amethystora-glass-surface {
  border: 1px solid rgba({{ accent_rgb }}, 0.75);
  box-shadow: 0 0 18px rgba({{ accent_rgb }}, 0.35);
}

#uiGroup.amethystora-style-neon .amethystora-glass-surface.amethystora-glass-menu { border-radius: 10px; }
#uiGroup.amethystora-style-neon .amethystora-glass-surface.amethystora-glass-panel { border-radius: 16px; }
#uiGroup.amethystora-style-neon .amethystora-glass-surface.amethystora-glass-inner { border-radius: 12px; }
#uiGroup.amethystora-style-neon .amethystora-glass-surface.amethystora-glass-banner { border-radius: 12px; }
#uiGroup.amethystora-style-neon .amethystora-glass-surface.amethystora-glass-osd { border-radius: 14px; }
#uiGroup.amethystora-style-neon .amethystora-glass-surface.amethystora-glass-dialog { border-radius: 12px; }
#uiGroup.amethystora-style-neon .amethystora-glass-surface.amethystora-glass-keyboard { border-radius: 10px; }
#uiGroup.amethystora-style-neon .message { border-radius: 10px; }

#uiGroup.amethystora-style-neon .popup-menu-item,
#uiGroup.amethystora-style-neon .amethystora-glass-surface .popup-sub-menu .popup-menu-item:last-child,
#uiGroup.amethystora-style-neon .button,
#uiGroup.amethystora-style-neon .modal-dialog-linked-button,
#uiGroup.amethystora-style-neon .notification-button,
#uiGroup.amethystora-style-neon .quick-toggle,
#uiGroup.amethystora-style-neon .quick-toggle-menu-button,
#uiGroup.amethystora-style-neon .switcher-list .item-box,
#uiGroup.amethystora-style-neon StEntry {
  border-radius: 8px;
}

#uiGroup.amethystora-style-neon .button:focus,
#uiGroup.amethystora-style-neon .modal-dialog-linked-button:focus,
#uiGroup.amethystora-style-neon .quick-toggle:checked,
#uiGroup.amethystora-style-neon StEntry:focus {
  box-shadow: 0 0 10px rgba({{ accent_rgb }}, 0.6);
}

#uiGroup.amethystora-style-neon #dash.amethystora-dock-dash .dash-background {
  border-radius: 16px;
  border-color: rgba({{ accent_rgb }}, 0.75);
  box-shadow: 0 0 18px rgba({{ accent_rgb }}, 0.35);
}

/* Quick settings (modules/quick.js): the blocks and buttons it adds take the corners each style gives GNOME's tiles */


#uiGroup.amethystora-style-hairline .quick-settings.amethystora-quick .quick-slider,
#uiGroup.amethystora-style-hairline .quick-settings.amethystora-quick .amethystora-quick-block,
#uiGroup.amethystora-style-hairline .quick-settings.amethystora-quick .amethystora-quick-action,
#uiGroup.amethystora-style-hairline .quick-settings.amethystora-quick .amethystora-quick-mute,
#uiGroup.amethystora-style-hairline .quick-settings.amethystora-quick .amethystora-quick-outputs,
#uiGroup.amethystora-style-hairline .quick-settings.amethystora-quick .amethystora-quick-output,
#uiGroup.amethystora-style-hairline .quick-settings.amethystora-quick .amethystora-quick-done,
#uiGroup.amethystora-style-hairline .amethystora-quick-limit {
  border-radius: 4px;
}

#uiGroup.amethystora-style-terminal .quick-settings.amethystora-quick .quick-slider,
#uiGroup.amethystora-style-terminal .quick-settings.amethystora-quick .amethystora-quick-block,
#uiGroup.amethystora-style-terminal .quick-settings.amethystora-quick .amethystora-quick-action,
#uiGroup.amethystora-style-terminal .quick-settings.amethystora-quick .amethystora-quick-mute,
#uiGroup.amethystora-style-terminal .quick-settings.amethystora-quick .amethystora-quick-outputs,
#uiGroup.amethystora-style-terminal .quick-settings.amethystora-quick .amethystora-quick-output,
#uiGroup.amethystora-style-terminal .quick-settings.amethystora-quick .amethystora-quick-done,
#uiGroup.amethystora-style-terminal .amethystora-quick-limit {
  border-radius: 0;
}

/* Monospaced text is wider: the longest name still fits its swatch, and Focus's line its row */
#uiGroup.amethystora-style-terminal .amethystora-quick-swatch-name {
  font-size: 10px;
}

#uiGroup.amethystora-style-terminal .amethystora-focus-line-text {
  font-size: 10.5px;
}

#uiGroup.amethystora-style-segments .quick-settings.amethystora-quick .quick-slider,
#uiGroup.amethystora-style-segments .quick-settings.amethystora-quick .amethystora-quick-block,
#uiGroup.amethystora-style-segments .quick-settings.amethystora-quick .amethystora-quick-action,
#uiGroup.amethystora-style-segments .quick-settings.amethystora-quick .amethystora-quick-mute,
#uiGroup.amethystora-style-segments .quick-settings.amethystora-quick .amethystora-quick-outputs,
#uiGroup.amethystora-style-segments .quick-settings.amethystora-quick .amethystora-quick-output,
#uiGroup.amethystora-style-segments .quick-settings.amethystora-quick .amethystora-quick-done,
#uiGroup.amethystora-style-segments .amethystora-quick-limit,
#uiGroup.amethystora-style-neon .quick-settings.amethystora-quick .quick-slider,
#uiGroup.amethystora-style-neon .quick-settings.amethystora-quick .amethystora-quick-block,
#uiGroup.amethystora-style-neon .quick-settings.amethystora-quick .amethystora-quick-action,
#uiGroup.amethystora-style-neon .quick-settings.amethystora-quick .amethystora-quick-mute,
#uiGroup.amethystora-style-neon .quick-settings.amethystora-quick .amethystora-quick-outputs,
#uiGroup.amethystora-style-neon .quick-settings.amethystora-quick .amethystora-quick-output,
#uiGroup.amethystora-style-neon .quick-settings.amethystora-quick .amethystora-quick-done,
#uiGroup.amethystora-style-neon .amethystora-quick-limit {
  border-radius: 8px;
}

/* A tile with an arrow is one card: its two parts are square where they meet and take the style's corners at its ends,
   as far round as a tile without one. The style's rules above give either part all four corners, and they beat the
   sided ones of stylesheets/quick.css. The card's own corners are the shield's ring. Terminal has no corners to give. */

#uiGroup.amethystora-style-islands .quick-toggle-has-menu .quick-toggle:ltr,
#uiGroup.amethystora-style-islands .quick-toggle-has-menu .quick-toggle-menu-button:rtl { border-radius: 12px 0 0 12px; }
#uiGroup.amethystora-style-islands .quick-toggle-has-menu .quick-toggle:rtl,
#uiGroup.amethystora-style-islands .quick-toggle-has-menu .quick-toggle-menu-button:ltr { border-radius: 0 12px 12px 0; }
#uiGroup.amethystora-style-islands .quick-toggle-has-menu,
#uiGroup.amethystora-style-islands .quick-toggle-has-menu .quick-toggle:last-child { border-radius: 12px; }

#uiGroup.amethystora-style-hairline .quick-toggle-has-menu .quick-toggle:ltr,
#uiGroup.amethystora-style-hairline .quick-toggle-has-menu .quick-toggle-menu-button:rtl { border-radius: 4px 0 0 4px; }
#uiGroup.amethystora-style-hairline .quick-toggle-has-menu .quick-toggle:rtl,
#uiGroup.amethystora-style-hairline .quick-toggle-has-menu .quick-toggle-menu-button:ltr { border-radius: 0 4px 4px 0; }
#uiGroup.amethystora-style-hairline .quick-toggle-has-menu,
#uiGroup.amethystora-style-hairline .quick-toggle-has-menu .quick-toggle:last-child { border-radius: 4px; }

#uiGroup.amethystora-style-capsule .quick-toggle-has-menu .quick-toggle:ltr,
#uiGroup.amethystora-style-capsule .quick-toggle-has-menu .quick-toggle-menu-button:rtl { border-radius: 999px 0 0 999px; }
#uiGroup.amethystora-style-capsule .quick-toggle-has-menu .quick-toggle:rtl,
#uiGroup.amethystora-style-capsule .quick-toggle-has-menu .quick-toggle-menu-button:ltr { border-radius: 0 999px 999px 0; }
#uiGroup.amethystora-style-capsule .quick-toggle-has-menu,
#uiGroup.amethystora-style-capsule .quick-toggle-has-menu .quick-toggle:last-child { border-radius: 999px; }

#uiGroup.amethystora-style-terminal .quick-toggle-has-menu { border-radius: 0; }

#uiGroup.amethystora-style-segments .quick-toggle-has-menu .quick-toggle:ltr,
#uiGroup.amethystora-style-segments .quick-toggle-has-menu .quick-toggle-menu-button:rtl,
#uiGroup.amethystora-style-neon .quick-toggle-has-menu .quick-toggle:ltr,
#uiGroup.amethystora-style-neon .quick-toggle-has-menu .quick-toggle-menu-button:rtl { border-radius: 8px 0 0 8px; }
#uiGroup.amethystora-style-segments .quick-toggle-has-menu .quick-toggle:rtl,
#uiGroup.amethystora-style-segments .quick-toggle-has-menu .quick-toggle-menu-button:ltr,
#uiGroup.amethystora-style-neon .quick-toggle-has-menu .quick-toggle:rtl,
#uiGroup.amethystora-style-neon .quick-toggle-has-menu .quick-toggle-menu-button:ltr { border-radius: 0 8px 8px 0; }
#uiGroup.amethystora-style-segments .quick-toggle-has-menu,
#uiGroup.amethystora-style-segments .quick-toggle-has-menu .quick-toggle:last-child,
#uiGroup.amethystora-style-neon .quick-toggle-has-menu,
#uiGroup.amethystora-style-neon .quick-toggle-has-menu .quick-toggle:last-child { border-radius: 8px; }

/* Neon's glow is the card's too, or the part under it would glow onto the arrow */
#uiGroup.amethystora-style-neon .quick-toggle-has-menu:checked { box-shadow: 0 0 10px rgba({{ accent_rgb }}, 0.6); }
#uiGroup.amethystora-style-neon .quick-toggle-has-menu .quick-toggle:checked { box-shadow: none; }

/* A glow would show through a tile that is not filled, so a tile with the keyboard has the ring every other style has */
#uiGroup.amethystora-style-neon .quick-toggle:focus,
#uiGroup.amethystora-style-neon .quick-toggle-has-menu .quick-toggle:focus { box-shadow: inset 0 0 0 2px rgba({{ foreground_rgb }}, 0.75); }
