// Keeping a feature's data under the account's key in Amethystora Vault, without ever holding that key (apps/vault-client is the
// same contract in Rust): a Unix stream at /run/amethystora/vault.sock, one line of JSON each way,
//   {"v":1,"op":"seal"|"open","purpose":..,"record":..,"data":"<base64>"}  ->  {"ok":true,"data":..} or {"ok":false,"why":..}
// `why` is closed (set up, and closed: say "Open Vault", never ask for it), not-set-up (work from memory, offer once), refused,
// too-large, bad-request or failed (as closed). A connection idle for five seconds is dropped, so each call makes its own. Nothing here
// blocks the shell: the connect, the write and the read are asynchronous, and a call that is not answered in time is cancelled.
//
// Called only after a person's own click (plans/done/PLAN-ui-ideas-r7-build.md, Keys): nothing asks the Vault unasked, and what is
// sealed is never kept in the clear when the Vault says no.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

export const SOCKET = '/run/amethystora/vault.sock';
// What one seal or open carries, before base64
export const MAX_DATA = 1024 * 1024;
const PURPOSE = /^[a-z0-9-]{1,40}$/;
const WAIT_MS = 10000;

// A test gives a stand-in socket here, and only here: it is never read from the environment or a setting
let socketPath = SOCKET;
export function useSocketForTest(path) {
    socketPath = path ?? SOCKET;
}

const refuse = why => ({ok: false, why});

// One request and its answer: a promise that never rejects, of {ok: true, data: Uint8Array} or {ok: false, why}
function ask(op, purpose, record, bytes, waitMs = WAIT_MS) {
    if (!PURPOSE.test(purpose) || record.length > 128 || /[\u0000-\u001f\u007f]/u.test(record))
        return Promise.resolve(refuse('bad-request'));
    if (bytes.length > MAX_DATA + 32)
        return Promise.resolve(refuse('too-large'));
    const line = `${JSON.stringify({v: 1, op, purpose, record, data: GLib.base64_encode(bytes)})}\n`;
    return new Promise(resolve => {
        const cancellable = new Gio.Cancellable();
        let done = false;
        let connection = null;
        let timer = 0;
        const finish = result => {
            if (done)
                return;
            done = true;
            if (timer)
                GLib.source_remove(timer);
            timer = 0;
            connection?.close_async(GLib.PRIORITY_DEFAULT, null, (c, res) => {
                try {
                    c.close_finish(res);
                } catch {}
            });
            resolve(result);
        };
        timer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, waitMs, () => {
            timer = 0;
            cancellable.cancel();
            finish(refuse('failed'));
            return GLib.SOURCE_REMOVE;
        });
        const read = () => {
            const input = new Gio.DataInputStream({base_stream: connection.get_input_stream(), close_base_stream: false});
            input.read_line_async(GLib.PRIORITY_DEFAULT, cancellable, (stream, res) => {
                try {
                    const [text] = stream.read_line_finish(res);
                    const answer = JSON.parse(new TextDecoder().decode(text));
                    if (answer.ok === true && typeof answer.data === 'string')
                        finish({ok: true, data: GLib.base64_decode(answer.data)});
                    else
                        finish(refuse(typeof answer.why === 'string' ? answer.why : 'failed'));
                } catch {
                    finish(refuse('failed'));
                }
            });
        };
        new Gio.SocketClient().connect_async(new Gio.UnixSocketAddress({path: socketPath}), cancellable, (client, res) => {
            try {
                connection = client.connect_finish(res);
            } catch {
                finish(refuse('failed'));
                return;
            }
            connection.get_output_stream().write_all_async(new TextEncoder().encode(line), GLib.PRIORITY_DEFAULT, cancellable,
                (stream, wrote) => {
                    try {
                        stream.write_all_finish(wrote);
                    } catch {
                        finish(refuse('failed'));
                        return;
                    }
                    read();
                });
        });
    });
}

// `plain` (Uint8Array) sealed under this account's key for `purpose`
export const seal = (purpose, record, plain, waitMs) => ask('seal', purpose, record, plain, waitMs);

// What seal() sealed, given the same purpose and record
export const open = (purpose, record, sealed, waitMs) => ask('open', purpose, record, sealed, waitMs);
