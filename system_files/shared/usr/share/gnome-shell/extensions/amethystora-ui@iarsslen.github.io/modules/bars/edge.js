// The parts of a screen's top edge that lead nowhere: the bar peeks over a fullscreen window when the pointer is pushed
// against them. A part another screen's bottom edge touches is the way up to that screen, and has no barrier, as
// GNOME's hot corners leave an edge with a neighbour alone. Plain arithmetic on {x, y, width, height}, with no import,
// so that it can be checked on its own (node build_files/shared/test-bars-edge.mjs).

// The [x1, x2) spans of `monitor`'s top edge that no monitor of `monitors` sits right above
export function freeTopEdge(monitor, monitors) {
    let spans = [[monitor.x, monitor.x + monitor.width]];
    for (const other of monitors) {
        if (other === monitor || other.y + other.height !== monitor.y)
            continue;
        const [a, b] = [other.x, other.x + other.width];
        spans = spans.flatMap(([x1, x2]) => {
            if (b <= x1 || a >= x2)
                return [[x1, x2]];
            const left = a > x1 ? [[x1, a]] : [];
            const right = b < x2 ? [[b, x2]] : [];
            return [...left, ...right];
        });
    }
    return spans.filter(([x1, x2]) => x2 > x1);
}

// The [x1, x2) spans of `monitor`'s bottom edge that no monitor of `monitors` sits right below: where the bar comes up
// from when it sits at the bottom
export function freeBottomEdge(monitor, monitors) {
    const flipped = m => ({x: m.x, y: -(m.y + m.height), width: m.width, height: m.height});
    const index = monitors.indexOf(monitor);
    const all = monitors.map(flipped);
    return freeTopEdge(all[index] ?? flipped(monitor), all);
}
