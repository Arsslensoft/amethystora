// Where a connection breaks, worked out from what the network menu's check found: nothing of the shell here, so that
// build_files/shared/test-truth.mjs runs it under node.
//
// A router may simply not answer pings, so its silence alone proves nothing: only silence and a name that does not
// resolve say the break may be at the router. A name server that answers says a way out exists, as far as this can tell.
// Where the machine cannot know (the connectivity check is off, resolved cannot be asked, the connection is not the one
// the internet goes through), it says less.

// The check's flags for resolved's ResolveHostname: DNS only (SD_RESOLVED_DNS, 1), never from the cache
// (SD_RESOLVED_NO_CACHE, 1 << 12): an old answer would call a dead connection fine.
export const NAME_FLAGS = 1 | (1 << 12);

// The name NetworkManager's own connectivity check already looks up, from the address that check fetches, so that the network
// and the name server learn nothing new. Null for an address with no name in it (a number), one that is not plain, or
// none at all. Nothing is fetched from it.
export function hostOf(uri) {
    const found = /^https?:\/\/([A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)+)(?::\d{1,5})?(?:[/?#]|$)/.exec(String(uri ?? ''));
    if (!found || found[1].length > 253 || /^[0-9.]+$/.test(found[1]))
        return null;
    return found[1].toLowerCase();
}

// Whether /proc/net/arp holds a finished entry for this address on this device: the router answered at the link, even if it
// ignores pings (an entry can outlive a router for a few minutes, so this says it answered lately, not now)
export function arpAnswers(text, address, device) {
    for (const line of String(text ?? '').split('\n').slice(1)) {
        const [ip, , flags, mac, , dev] = line.trim().split(/\s+/);
        if (ip === address && dev === device && flags === '0x2' && mac && mac !== '00:00:00:00:00:00')
            return true;
    }
    return false;
}

// What a failed lookup means, from the D-Bus error's remote name: "no such name" came from a name server that answers.
// 'unknown': resolved itself could not be asked, which says nothing about the network.
export function nameFailure(remote) {
    if (typeof remote !== 'string')
        return 'fail';
    if (remote.endsWith('.DnsError.NXDOMAIN'))
        return 'ok';
    if (remote.startsWith('org.freedesktop.resolve1.'))
        return 'fail';
    if (remote === 'org.freedesktop.DBus.Error.ServiceUnknown' || remote === 'org.freedesktop.DBus.Error.NameHasNoOwner')
        return 'unknown';
    return 'fail';
}

// router: 'wait', 'ok' (answered a ping), 'arp' (no ping answer, but it answers at the link), 'silent' or 'none' (the
//   connection has no gateway)
// name: 'wait', 'ok', 'fail', 'unknown' (resolved could not be asked) or 'skip' (not asked: NetworkManager's own check is off)
// connectivity: what NetworkManager says of this connection: 'full', 'limited', 'portal', 'none' or 'unknown'
// main: whether this is the connection the internet goes through. NetworkManager's check and the name lookup are the
//   machine's, not this link's: for any other, only the router is asked and only the first line is drawn.
// Each line of the drawing is 'wait', 'ok', 'warn', 'bad' or 'off'; say is which sentence to show.
export function verdict({router, name, connectivity, main = true}) {
    const reached = router === 'ok' || router === 'arp';
    if (!main) {
        if (router === 'wait')
            return {first: 'wait', second: 'off', say: 'checking'};
        if (router === 'none')
            return {first: 'off', second: 'off', say: 'side-none'};
        return reached ? {first: 'ok', second: 'off', say: 'side-ok'} : {first: 'warn', second: 'off', say: 'side-silent'};
    }
    if (connectivity === 'portal' && router !== 'wait')
        return {first: 'ok', second: 'warn', say: 'portal'};
    if (router === 'wait' || name === 'wait')
        return {first: reached ? 'ok' : 'wait', second: 'wait', say: 'checking'};
    if (name === 'ok') {
        if (connectivity === 'limited' || connectivity === 'none')
            return {first: 'ok', second: 'warn', say: 'limited'};
        return {first: 'ok', second: 'ok', say: 'fine'};
    }
    if (name === 'fail') {
        // NetworkManager's check got through (its word can be minutes old): the way out works and the names do not
        if (connectivity === 'full')
            return {first: 'ok', second: 'warn', say: 'dns'};
        if (router === 'none')
            return {first: 'bad', second: 'off', say: 'no-gateway'};
        return reached ? {first: 'ok', second: 'bad', say: 'beyond'} : {first: 'bad', second: 'off', say: 'router'};
    }
    // The name could not be asked (resolved), or was not (NetworkManager's check is off): NetworkManager's word is all there is
    if (connectivity === 'full')
        return {first: 'ok', second: 'ok', say: 'fine'};
    if (router === 'none')
        return {first: 'off', second: 'off', say: 'no-gateway'};
    if (!reached)
        return {first: 'warn', second: 'off', say: 'silent'};
    if (connectivity === 'limited' || connectivity === 'none')
        return {first: 'ok', second: 'bad', say: 'beyond'};
    return {first: 'ok', second: 'off', say: 'router-only'};
}
