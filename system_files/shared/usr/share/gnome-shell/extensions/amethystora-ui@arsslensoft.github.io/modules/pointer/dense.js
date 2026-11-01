// Whether a screen's pixels are small enough for the pointer to grow a size: a screen shown at 100% whose density is
// a quarter above the one GNOME aims its own scaling at (135 dots an inch under 20 inches, 110 from there). A 14-inch
// 2560x1600 laptop panel or a 27-inch 4K monitor is; a 14-inch 1920x1200 panel or a 27-inch 2560x1440 monitor is not.
// A screen GNOME already scales draws a larger pointer by itself. No GNOME imports, so that the rule can be tried
// outside the shell.

// Sizes some screens report in place of their size: their shape, in centimetres or millimetres. GNOME ignores them too.
export const ASPECT_AS_SIZE = [[1600, 900], [1600, 1000], [160, 90], [160, 100], [16, 9], [16, 10]];

const MOBILE_DPI = 135;
const LARGE_DPI = 110;
const LARGE_INCHES = 20;
export const DENSE = 1.25; // how far above GNOME's aim a screen has to be
export const LARGER = 32; // the size one step above GNOME's 24

// The pointer size for a screen, or null to leave the size as it is
export function denseSize({widthPx, heightPx, widthMm, heightMm, scale, tiled}) {
    if (scale !== 1 || tiled || !(widthMm > 0) || !(heightMm > 0))
        return null;
    if (ASPECT_AS_SIZE.some(([w, h]) => w === widthMm && h === heightMm))
        return null;
    const inches = Math.hypot(widthMm, heightMm) / 25.4;
    const dpi = Math.hypot(widthPx, heightPx) / inches;
    const target = inches < LARGE_INCHES ? MOBILE_DPI : LARGE_DPI;
    return dpi >= DENSE * target ? LARGER : null;
}
