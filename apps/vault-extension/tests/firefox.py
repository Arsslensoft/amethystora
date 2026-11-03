#!/usr/bin/python3
"""firefox.py: the extension in Fedora's Firefox, through Marionette, with the real native host and a stand-in for the Vault's service.

Run as root in a throwaway Fedora container that has `firefox`, with the host built (it is installed at its real path, and the stand-in
listens on the real socket, /run/amethystora/vault.sock):

    python3 -I apps/vault-extension/tests/firefox.py /path/to/amethystora-vault-native

Prints one line per check and exits 1 if any failed. The checks themselves are lab.py's, which chromium.py runs too. The real service,
polkit and a real desktop are suite 74's (plans/vault/work/ext.md).
"""
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lab  # noqa: E402
from lab import BASE, EXTENSION, GRANTED, PORT, check, ops, wait  # noqa: E402

ADDON_ID = 'vault@amethystora.org'

GRANT = """
const [id, origins, done] = arguments;
const { ExtensionPermissions } = ChromeUtils.importESModule('resource://gre/modules/ExtensionPermissions.sys.mjs');
const policy = WebExtensionPolicy.getByID(id);
ExtensionPermissions.add(id, { permissions: [], origins }, policy.extension).then(() => done(true), (error) => done(String(error)));
"""

# The toolbar's popup is a panel of the browser window, in the browser's own interface
POPUP_PANEL = """
const [done] = arguments;
const window = Services.wm.getMostRecentWindow('navigator:browser');
done(Boolean(window.document.querySelector('browser[webextension-view-type="popup"]')));
"""


HIDE_PANEL = """
const [done] = arguments;
const window = Services.wm.getMostRecentWindow('navigator:browser');
for (const view of window.document.querySelectorAll('browser[webextension-view-type="popup"]')) {
  view.closest('panel')?.hidePopup();
}
done(true);
"""


def main():
    host_binary = sys.argv[1]
    os.makedirs('/usr/libexec', exist_ok=True)
    shutil.copy(host_binary, '/usr/libexec/amethystora-vault-native')
    lab.add_hosts()
    home = tempfile.mkdtemp(prefix='vault-firefox-')
    environment = dict(os.environ, HOME=home, MOZ_REMOTE_ALLOW_SYSTEM_ACCESS='1', MOZ_DISABLE_CONTENT_SANDBOX='1', MOZ_DISABLE_GMP_SANDBOX='1',
                       MOZ_DISABLE_RDD_SANDBOX='1', MOZ_DISABLE_SOCKET_PROCESS_SANDBOX='1', MOZ_HEADLESS_WIDTH='1280', MOZ_HEADLESS_HEIGHT='900')
    subprocess.run(['/usr/libexec/amethystora-vault-native', 'browser', 'on'], env=environment, check=True)
    lab.serve_vault()
    lab.serve_pages()
    profile = os.path.join(home, 'profile')
    os.makedirs(profile)
    with open(os.path.join(profile, 'user.js'), 'w') as prefs:
        for name, value in {'marionette.port': 2828, 'network.proxy.type': 0, 'browser.shell.checkDefaultBrowser': False, 'datareporting.policy.dataSubmissionEnabled': False,
                            'extensions.autoDisableScopes': 0, 'network.dns.disableIPv6': True, 'security.sandbox.content.level': 0, 'dom.security.https_only_mode': False}.items():
            prefs.write(f'user_pref({json.dumps(name)}, {json.dumps(value)});\n')
    firefox = subprocess.Popen(['firefox', '--headless', '--marionette', '--remote-allow-system-access', '--no-remote', '-profile', profile],
                               env=environment, stdout=subprocess.DEVNULL, stderr=open(os.path.join(home, 'firefox.log'), 'w'))
    try:
        browser = lab.Marionette(2828)
        browser.call('WebDriver:NewSession', {'capabilities': {}})
        print('Firefox', browser.chrome('arguments[0](Services.appinfo.version)'), flush=True)
        browser.call('Addon:Install', {'path': EXTENSION, 'temporary': True})
        browser.call('WebDriver:SetWindowRect', {'width': 1280, 'height': 900})
        run(browser)
    except BaseException:
        print('Firefox log:', open(os.path.join(home, 'firefox.log')).read()[-3000:], flush=True)
        raise
    finally:
        firefox.terminate()
        firefox.wait(timeout=20)
    print(f'{lab.checks} checks, {len(lab.failures)} failed', flush=True)
    if lab.failures:
        print('Firefox log:', open(os.path.join(home, 'firefox.log')).read()[-3000:])
    sys.exit(1 if lab.failures else 0)


def run(browser):
    # Before any site is granted: nothing in the page, the host never started
    browser.go(BASE + '/login.html')
    browser.click_element('#user')
    time.sleep(0.5)
    check('without a grant, no button and no host', not lab.has_button(browser) and ops() == [])
    # The person grants the site (in the popup: the browser asks); here Marionette grants it as that would
    granted = browser.chrome(GRANT, [ADDON_ID, GRANTED])
    check('the grant', granted is True, str(granted))
    time.sleep(1)
    uuid = browser.chrome("const [done] = arguments; done(WebExtensionPolicy.getByID('vault@amethystora.org').mozExtensionHostname);")
    extension_url = f'moz-extension://{uuid}'

    def popup_opened():
        return browser.chrome(POPUP_PANEL) is True or len(browser.handles()) > 1

    lab.hide_toolbar_popup = lambda: browser.chrome(HIDE_PANEL)
    lab.run(browser, extension_url, popup_opened)
    cost(browser)


COST = """
const [code, done] = arguments;
const compileStarted = performance.now();
const start = new Function(code);
const compiled = performance.now() - compileStarted;
const runs = 200;
const started = performance.now();
for (let n = 0; n < runs; n++) {
  globalThis.vaultButton = undefined;
  start();
}
done([compiled, (performance.now() - started) / runs]);
"""


def cost(browser):
    browser.go(f'http://quiet.example.net:{PORT}/quiet.html')
    nodes = browser.page('return document.getElementsByTagName("*").length')
    # The content script's own work as it starts, in a sandbox of its own like the extension's world, the extension APIs aside
    code = open(os.path.join(EXTENSION, 'fields.js')).read() + '\n' + open(os.path.join(EXTENSION, 'content.js')).read()
    compiled, each = browser.call('WebDriver:ExecuteAsyncScript', {'script': COST, 'args': [code], 'sandbox': 'measure'})
    print(f'       content script on a page of {nodes} elements with no login field: compiling {compiled * 1000:.0f} us once (the browser caches it), '
          f'running {each * 1000:.1f} us (mean of 200)', flush=True)
    check('running it costs under 0.5 ms on a page with no login field', each < 0.5, f'{each} ms')
    # Focus moving between fields that are not logins: no button, and the host never starts
    before = len(lab.heard)
    for target in ['#search', '#notes', '#search']:
        browser.click_element(target)
    time.sleep(0.5)
    check('a page with no login field: no button, host never started', not lab.has_button(browser) and len(lab.heard) == before)


if __name__ == '__main__':
    main()
