// What CUPS says of a print job, as data: nothing of the shell here, so that build_files/shared/test-devices.mjs reads
// the same code under node. The answer comes from ipptool -c with /usr/share/amethystora/print/jobs-state.test, one Get-Jobs
// for the jobs of a printer, which asks for states, reasons and page counts and never for the document's name or the
// printer's own words.

// Keywords the printer reports, by what they mean to a person: the first that matches says what is wrong. A -warning is
// never trouble (toner low still prints), and a -report only when the printer has gone away. connecting-to-device is not
// here: the network backends set it during every normal connect, so it says nothing is wrong.
const TROUBLE = [
    ['paper', ['media-empty', 'media-needed', 'input-tray-missing']],
    ['jam', ['media-jam']],
    ['ink', ['toner-empty', 'marker-supply-empty', 'marker-waste-full']],
    ['cover', ['door-open', 'cover-open', 'interlock-open']],
    ['offline', ['offline', 'shutdown', 'timed-out']],
];
const GONE_AWAY = new Set(['offline', 'shutdown', 'timed-out']);
const WORD = /^[a-z0-9-]{1,80}$/;

// A job in one of these will not change again
export const OVER = new Set(['canceled', 'aborted', 'completed']);

// One CSV line as ipptool -c writes it, quotes and all; a list of values is one field
function fields(line) {
    const found = [];
    let field = '';
    let quoted = false;
    for (let i = 0; i < line.length; i++) {
        const c = line[i];
        if (c === '"') {
            if (quoted && line[i + 1] === '"') {
                field += '"';
                i++;
            } else {
                quoted = !quoted;
            }
        } else if (c === ',' && !quoted) {
            found.push(field);
            field = '';
        } else {
            field += c;
        }
    }
    found.push(field);
    return found;
}

function row(lines, name) {
    const at = lines.findIndex(line => fields(line).includes(name));
    if (at < 0 || at + 1 >= lines.length)
        return null;
    const names = fields(lines[at]);
    const values = fields(lines[at + 1]);
    return Object.fromEntries(names.map((key, i) => [key, values[i] ?? '']));
}

function keywords(value) {
    return (value ?? '').split(',').map(word => word.trim()).filter(word => WORD.test(word));
}

function count(value) {
    const number = Number.parseInt(value, 10);
    return Number.isInteger(number) && number >= 0 ? number : null;
}

function linesOf(text) {
    return (text ?? '').split('\n').map(line => line.replace(/\r$/, '')).filter(Boolean);
}

// One job's view, from its fields and the printer's: {state, reasons, done, total, printerState, printerReasons}
function viewOf(job, printer) {
    const done = count(job['job-impressions-completed']) ?? count(job['job-media-sheets-completed']);
    let total = count(job['job-impressions']) ?? count(job['job-media-sheets']);
    // Nothing printed yet, or a count that has already been passed, says nothing of how long the job is
    if (!total || (done !== null && done > total))
        total = null;
    return {
        state: job['job-state'],
        reasons: keywords(job['job-state-reasons']),
        done: done || null,
        total,
        printerState: WORD.test(printer?.['printer-state'] ?? '') ? printer['printer-state'] : '',
        printerReasons: keywords(printer?.['printer-state-reasons'] ?? job['job-printer-state-reasons']),
    };
}

// The view of one job, or null when CUPS has no such job for this account
export function parseState(text) {
    const lines = linesOf(text);
    const job = row(lines, 'job-state');
    if (!job || !WORD.test(job['job-state'] ?? ''))
        return null;
    return viewOf(job, row(lines, 'printer-state'));
}

// The views of several jobs of one printer, from one Get-Jobs answer: a Map from the job's number to its view. A number
// that is not in the answer is not in the Map.
export function parseStates(text) {
    const lines = linesOf(text);
    const found = new Map();
    const at = lines.findIndex(line => {
        const names = fields(line);
        return names.includes('job-id') && names.includes('job-state');
    });
    if (at < 0)
        return found;
    const names = fields(lines[at]);
    const printer = row(lines, 'printer-state');
    for (const line of lines.slice(at + 1)) {
        const values = fields(line);
        if (values.includes('printer-state'))
            break;
        const job = Object.fromEntries(names.map((key, i) => [key, values[i] ?? '']));
        const id = Number.parseInt(job['job-id'], 10);
        if (Number.isInteger(id) && id > 0 && WORD.test(job['job-state'] ?? ''))
            found.set(id, viewOf(job, printer));
    }
    return found;
}

// What is wrong, as one word, or null when nothing is. Words a printer reports that are not in the table say nothing:
// a printer that reports nothing useful prints with "Printing" and no more.
export function trouble(view) {
    if (!view)
        return null;
    for (const reason of view.printerReasons) {
        if (reason.endsWith('-warning'))
            continue;
        const report = reason.endsWith('-report');
        const base = reason.replace(/-(error|report)$/, '');
        if (report && !GONE_AWAY.has(base))
            continue;
        const found = TROUBLE.find(([, list]) => list.includes(base));
        if (found)
            return found[0];
    }
    if (view.printerState === 'stopped' || view.printerReasons.includes('paused'))
        return 'stopped';
    if (view.state === 'processing-stopped')
        return 'job-stopped';
    if (view.state === 'pending-held')
        return 'held';
    return null;
}

// What Try again does for a job in this state, as an argument vector, or null when there is nothing to try: paper added
// or a cover closed carries on by itself. The printer's name and the job's number have been checked by the caller.
export function retryCommand(view, id, printer) {
    switch (trouble(view)) {
    case 'stopped':
        return ['/usr/bin/cupsenable', printer];
    case 'job-stopped':
    case 'held':
        return ['/usr/bin/lp', '-i', String(id), '-H', 'resume'];
    default:
        return null;
    }
}

// The part of a signal CUPS sends that can be used: a printer's name fit for an address (and, with no leading dash, never
// an option of cupsenable) and a job's number
export function validJob(printer, id) {
    return typeof printer === 'string' && /^[^\s/#\\"'?%-][^\s/#\\"'?%]{0,126}$/.test(printer) &&
        Number.isInteger(id) && id > 0;
}

// The most print jobs followed at once, which is also how many one ask carries (the places in jobs-state.test)
export const MAX_JOBS = 16;

// Whether a signal's job is followed, and if so which: the owner is the account's, the names are fit, and no more than
// MAX_JOBS are followed (the rest are the printer's business). Puts it in `jobs` (number to printer) and says whether it did.
export function admit(jobs, printer, id, owner, user) {
    if (owner !== user || !validJob(printer, id))
        return false;
    if (!jobs.has(id) && jobs.size >= MAX_JOBS)
        return false;
    jobs.set(id, printer);
    return true;
}

// The followed jobs by printer: a Map from the printer to the numbers of its jobs
export function byPrinter(jobs) {
    const groups = new Map();
    for (const [id, printer] of jobs) {
        if (!groups.has(printer))
            groups.set(printer, []);
        groups.get(printer).push(id);
    }
    return groups;
}

// The ipptool command that asks for all these jobs of one printer in one request: the numbers go in the places of
// jobs-state.test, the last number repeating when there are fewer
export function askArgv(tool, question, user, printer, ids) {
    const argv = [tool, '-c', '-T', '10', '-d', `user=${user}`];
    for (let n = 0; n < MAX_JOBS; n++)
        argv.push('-d', `j${n + 1}=${ids[n] ?? ids[0]}`);
    argv.push(`ipp://localhost/printers/${encodeURIComponent(printer)}`, question);
    return argv;
}
