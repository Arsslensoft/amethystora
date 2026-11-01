/* Living wallpapers in the colours of {{ theme_name }} (modules/living.js reads these three from the layer's style):
   the ground is the theme's background, the accent its accent, and the second colour its blue. The selector starts
   from the shell's top group, one id more than the module's own stylesheet, which stands in until a theme is applied. */

#uiGroup .amethystora-living {
  -amethystora-living-ground: {{ background }};
  -amethystora-living-accent: {{ accent }};
  -amethystora-living-second: {{ color4 }};
}
