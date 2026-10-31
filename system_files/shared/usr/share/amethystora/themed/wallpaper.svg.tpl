<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="3840" height="2160" viewBox="0 0 3840 2160">
  <!--
  The wallpaper a theme falls back to when it ships no backgrounds/ directory: the palette's own
  background, lit from the upper left in its accent. Rendered from colors.toml, so a theme added by
  hand gets a matching wallpaper without anyone drawing one.
  -->
  <defs>
    <radialGradient id="glow" cx="0.28" cy="0.22" r="0.85">
      <stop offset="0" stop-color="{{ accent }}" stop-opacity="0.55"/>
      <stop offset="0.45" stop-color="{{ color5 }}" stop-opacity="0.18"/>
      <stop offset="1" stop-color="{{ background }}" stop-opacity="0"/>
    </radialGradient>
    <radialGradient id="counterglow" cx="0.82" cy="0.86" r="0.7">
      <stop offset="0" stop-color="{{ color4 }}" stop-opacity="0.28"/>
      <stop offset="1" stop-color="{{ background }}" stop-opacity="0"/>
    </radialGradient>
    <linearGradient id="depth" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="{{ color0 }}" stop-opacity="0.35"/>
      <stop offset="1" stop-color="{{ background }}" stop-opacity="0.9"/>
    </linearGradient>
  </defs>
  <rect width="3840" height="2160" fill="{{ background }}"/>
  <rect width="3840" height="2160" fill="url(#depth)"/>
  <rect width="3840" height="2160" fill="url(#glow)"/>
  <rect width="3840" height="2160" fill="url(#counterglow)"/>
</svg>
