// The units the search converts between, for calc.js. Each has the symbol it is written with in an answer first, then
// any other symbols (matched as written, since "Mb" and "MB" are not the same), the names a person may type (matched
// whatever their case), what it measures, and its size in that measure's base unit (the metre, the kilogram, the
// byte, the second). Every factor is exact by definition. Temperatures are not a size but a scale, so theirs are
// worked out in kelvin by calc.js. No currency: its rates would have to come from the internet. No month or year:
// neither has a fixed length. Kept free of GNOME Shell's imports, so that it runs under node and gjs alike.

const unit = (symbols, names, dimension, factor) => ({symbol: symbols[0], symbols, names, dimension, factor});

// The names of a unit, singular and plural, in both spellings where British and American English differ
const both = (...stems) => stems.flatMap(stem => [stem, `${stem}s`]);
const metric = stem => both(`${stem}metre`, `${stem}meter`);

export const UNITS = [
    // Length, in metres
    unit(['mm'], metric('milli'), 'length', 0.001),
    unit(['cm'], metric('centi'), 'length', 0.01),
    unit(['m'], metric(''), 'length', 1),
    unit(['km'], metric('kilo'), 'length', 1000),
    unit(['in', '"'], ['inch', 'inches'], 'length', 0.0254),
    unit(['ft', '\''], ['foot', 'feet'], 'length', 0.3048),
    unit(['yd'], both('yard'), 'length', 0.9144),
    unit(['mi'], both('mile'), 'length', 1609.344),
    unit(['nmi'], both('nautical mile'), 'length', 1852),

    // Mass, in kilograms
    unit(['mg'], both('milligram'), 'mass', 1e-6),
    unit(['g'], both('gram'), 'mass', 0.001),
    unit(['kg'], both('kilogram'), 'mass', 1),
    unit(['t'], both('tonne'), 'mass', 1000),
    unit(['oz'], both('ounce'), 'mass', 0.028349523125),
    unit(['lb', 'lbs'], both('pound'), 'mass', 0.45359237),
    unit(['st'], both('stone'), 'mass', 6.35029318),

    // Temperature: a scale, see calc.js
    unit(['°C', 'C'], ['celsius', '°celsius'], 'temperature', null),
    unit(['°F', 'F'], ['fahrenheit', '°fahrenheit'], 'temperature', null),
    unit(['K'], ['kelvin', 'kelvins'], 'temperature', null),

    // Data, in bytes: bits and the decimal multiples, then the binary ones
    unit(['b', 'bit'], ['bits'], 'data', 1 / 8),
    unit(['kb', 'kbit'], both('kilobit'), 'data', 125),
    unit(['Mb', 'Mbit'], both('megabit'), 'data', 125e3),
    unit(['Gb', 'Gbit'], both('gigabit'), 'data', 125e6),
    unit(['Tb', 'Tbit'], both('terabit'), 'data', 125e9),
    unit(['B'], both('byte'), 'data', 1),
    unit(['kB', 'KB'], both('kilobyte'), 'data', 1e3),
    unit(['MB'], both('megabyte'), 'data', 1e6),
    unit(['GB'], both('gigabyte'), 'data', 1e9),
    unit(['TB'], both('terabyte'), 'data', 1e12),
    unit(['PB'], both('petabyte'), 'data', 1e15),
    unit(['KiB'], both('kibibyte'), 'data', 1024),
    unit(['MiB'], both('mebibyte'), 'data', 1024 ** 2),
    unit(['GiB'], both('gibibyte'), 'data', 1024 ** 3),
    unit(['TiB'], both('tebibyte'), 'data', 1024 ** 4),
    unit(['PiB'], both('pebibyte'), 'data', 1024 ** 5),

    // Time, in seconds
    unit(['ms'], both('millisecond'), 'time', 0.001),
    unit(['s', 'sec'], both('second'), 'time', 1),
    unit(['min'], both('minute'), 'time', 60),
    unit(['h', 'hr'], both('hour'), 'time', 3600),
    unit(['d'], both('day'), 'time', 86400),
    unit(['wk'], both('week'), 'time', 604800),
];

const BY_SYMBOL = new Map();
const BY_NAME = new Map();
for (const entry of UNITS) {
    for (const symbol of entry.symbols)
        BY_SYMBOL.set(symbol, entry);
    for (const name of entry.names)
        BY_NAME.set(name, entry);
}

// The unit a word stands for, or null: a symbol as written, else a name in any case
export function findUnit(word) {
    return BY_SYMBOL.get(word) ?? BY_NAME.get(word.toLowerCase()) ?? null;
}

// A temperature to kelvin and back
const KELVIN = {
    '°C': [x => x + 273.15, k => k - 273.15],
    '°F': [x => (x - 32) * 5 / 9 + 273.15, k => (k - 273.15) * 9 / 5 + 32],
    'K': [x => x, k => k],
};

// A value in one unit, in the other; both measure the same thing
export function convert(value, from, to) {
    if (from.dimension === 'temperature')
        return KELVIN[to.symbol][1](KELVIN[from.symbol][0](value));
    return value * from.factor / to.factor;
}
