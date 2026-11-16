// What the agent's tool server is doing, in a few words (R7-13): the server writes the name of the read tool it runs, and for
// `state` the topic, into its own session file; this reads that file and says it. Only the names of the tools' own lists are
// ever shown, never what was asked or read. No imports: Node reads this file in build_files/shared/test-agent-doing.mjs.

// The tools that read; any other name in the file is left out
export const TOOLS = ['search', 'read', 'state', 'logs', 'history'];
// `state`'s topics, as the tools list them (apps/agent-tools/src/state.rs TOPICS)
export const TOPICS = ['overview', 'health', 'settings', 'security', 'security-settings', 'app-permissions', 'software', 'updates',
    'backups', 'disks', 'setup-diff', 'units', 'crashes'];
// Seconds a finished step stays in the pill with its tick, so that the line does not flicker between two calls
export const LINGER = 4;
// Steps older than this many seconds are not a thing it is doing: a server that died unseen leaves its file behind
export const STALE = 300;

// The agents a session or a question may name (the launcher's own ids), and their names for people: the makers' product names
export const AGENT_TITLES = {
    claude: 'Claude Code', opencode: 'Opencode', codex: 'Codex', agy: 'Antigravity CLI', copilot: 'Copilot CLI',
    'cursor-agent': 'Cursor CLI',
};

// An agent id of the fixed list, else null: a file the account can write says it, so nothing else is taken
export function agentOf(value) {
    return typeof value === 'string' && Object.hasOwn(AGENT_TITLES, value) ? value : null;
}

// A profile's name as the tools make them, else null
export function profileOf(value) {
    return typeof value === 'string' && /^[a-z0-9-]{1,32}$/.test(value) ? value : null;
}

// Whether a sandbox is open or closed, else null
function sandboxOf(value) {
    return value === 'open' || value === 'closed' ? value : null;
}

// Who a session or a question is, in a line: the agent's name (`fallback` when the file does not name one), the profile unless
// it is the default, and "sandboxed". `_` is the shell's gettext.
export function who({agent, profile, sandboxed}, fallback, _) {
    const parts = [AGENT_TITLES[agent] ?? fallback];
    if (profile && profile !== 'default')
        parts.push(profile);
    if (sandboxed)
        parts.push(_('sandboxed'));
    return parts.join(' · ');
}

function step(value) {
    if (!value || typeof value !== 'object' || !TOOLS.includes(value.tool))
        return null;
    const at = Number(value.at);
    return {tool: value.tool, topic: value.tool === 'state' && TOPICS.includes(value.topic) ? value.topic : null,
        at: Number.isFinite(at) ? at : 0};
}

// A session file's `now` and `recent`, and who it is (`agent`, `profile`, `sandboxed`), checked: unknown names, a wrong shape or a
// broken file give nothing. A sandbox's runner writes the same file as a tool server, with `sandboxed` and `project` as well; the
// project is never read here.
export function parse(text) {
    const none = {now: null, recent: [], agent: null, profile: null, sandboxed: null};
    let body;
    try {
        body = JSON.parse(text);
    } catch {
        return none;
    }
    if (!body || typeof body !== 'object')
        return none;
    const recent = (Array.isArray(body.recent) ? body.recent : []).map(step).filter(Boolean).slice(-3);
    return {now: step(body.now), recent, agent: agentOf(body.agent), profile: profileOf(body.profile), sandboxed: sandboxOf(body.sandboxed)};
}

// The sessions the card lists one by one, or none: only when more than one runs, or one is on a profile other than the default, or
// one is sandboxed. `sessions` is a list of {pid, agent, profile, sandboxed}.
export function listed(sessions) {
    const any = sessions.some(s => s.sandboxed || (s.profile && s.profile !== 'default'));
    return sessions.length > 1 || any ? sessions : [];
}

// What the pill says: `now` of the session that is running a tool, else the most recent finished step of any session still
// within LINGER seconds. `sessions` is a list of parse() results, `at` the time in seconds.
export function pick(sessions, at) {
    const running = sessions.map(s => s.now).filter(n => n && at - n.at < STALE).sort((a, b) => b.at - a.at)[0];
    if (running)
        return {running: true, step: running};
    const done = sessions.flatMap(s => s.recent).filter(n => at - n.at < LINGER).sort((a, b) => b.at - a.at)[0];
    return done ? {running: false, step: done} : null;
}

// The seconds until the finished step in `picked` is over (so that one timer ends it), or 0
export function lingerLeft(picked, at) {
    return picked && !picked.running ? Math.max(0, LINGER - (at - picked.step.at)) : 0;
}

// The steps for the card, oldest first: what finished (up to three, from every session) and what runs now
export function rows(sessions, at) {
    const done = sessions.flatMap(s => s.recent).sort((a, b) => a.at - b.at).slice(-3);
    const now = sessions.map(s => s.now).filter(n => n && at - n.at < STALE).sort((a, b) => a.at - b.at).at(-1) ?? null;
    return {done, now};
}

// The words, from `_` (the shell's gettext). `present` for what runs now, else for what is done.
export function phrase(item, present, _) {
    const topic = {
        overview: [_('Looking over this computer'), _('Looked over this computer')],
        health: [_('Checking its health'), _('Checked its health')],
        settings: [_('Reading the settings'), _('Read the settings')],
        security: [_('Reading the security report'), _('Read the security report')],
        'security-settings': [_('Reading the security settings'), _('Read the security settings')],
        'app-permissions': [_('Checking what apps may reach'), _('Checked what apps may reach')],
        software: [_('Looking at the software'), _('Looked at the software')],
        updates: [_('Checking the updates'), _('Checked the updates')],
        backups: [_('Checking the backups'), _('Checked the backups')],
        disks: [_('Checking the disks'), _('Checked the disks')],
        'setup-diff': [_('Comparing your setup'), _('Compared your setup')],
        units: [_('Checking the services'), _('Checked the services')],
        crashes: [_('Looking at the crashes'), _('Looked at the crashes')],
    };
    const tool = {
        search: [_('Searching the manual'), _('Searched the manual')],
        read: [_('Reading the manual'), _('Read the manual')],
        state: [_('Checking this computer'), _('Checked this computer')],
        logs: [_('Reading the logs'), _('Read the logs')],
        history: [_('Looking at what changed'), _('Looked at what changed')],
    };
    const words = (item.topic && topic[item.topic]) || tool[item.tool];
    return words ? words[present ? 0 : 1] : '';
}
