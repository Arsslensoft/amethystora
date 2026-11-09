// A widget's style classes changed in one go. St restyles a widget and everything under it at each change of its classes,
// at once when it is on the stage: two classes taken off and one put on one at a time cost three restyles (some 30 ms
// each for the top bar), and this one.
export function setClasses(widget, add = [], remove = []) {
    const had = (widget.get_style_class_name() ?? '').split(/\s+/).filter(Boolean);
    const kept = had.filter(name => !remove.includes(name));
    const added = add.filter(name => !kept.includes(name));
    if (!added.length && kept.length === had.length)
        return;
    widget.set_style_class_name([...kept, ...added].join(' ') || null);
}
