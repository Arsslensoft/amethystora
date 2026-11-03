#!/usr/bin/python3
"""chromium.py: the extension in Fedora's Chromium, through chromedriver, with the real native host and a stand-in for the Vault's service.
The same checks as firefox.py (they are lab.py's).

Run as root in a throwaway Fedora container that has `chromium` and `chromedriver`, with the host built:

    python3 -I apps/vault-extension/tests/chromium.py /path/to/amethystora-vault-native

The extension is loaded unpacked from a copy of its folder whose manifest gains the test key (tests/chromium-key.txt, which gives it the
placeholder id `ame vault browser on` writes) and host permissions for the test sites, since a headless browser cannot be asked to grant
them: so "without a site granted, nothing is drawn" is firefox.py's alone. Prints one line per check and exits 1 if any failed.
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
from lab import EXTENSION, GRANTED, HERE  # noqa: E402

# The id of tests/chromium-key.txt, the placeholder in install.rs's CHROMIUM_IDS
EXTENSION_ID = 'fcfklbbmhabecjliaimhlhpiklpdacno'
DRIVER_PORT = 9515


def main():
    host_binary = sys.argv[1]
    os.makedirs('/usr/libexec', exist_ok=True)
    shutil.copy(host_binary, '/usr/libexec/amethystora-vault-native')
    lab.add_hosts()
    home = tempfile.mkdtemp(prefix='vault-chromium-')
    environment = dict(os.environ, HOME=home)
    subprocess.run(['/usr/libexec/amethystora-vault-native', 'browser', 'on'], env=environment, check=True)
    lab.serve_vault()
    lab.serve_pages()

    unpacked = os.path.join(home, 'extension')
    shutil.copytree(EXTENSION, unpacked, ignore=shutil.ignore_patterns('tests', 'tools'))
    manifest_path = os.path.join(unpacked, 'manifest.json')
    manifest = json.load(open(manifest_path))
    manifest['key'] = open(os.path.join(HERE, 'chromium-key.txt')).read().strip()
    manifest['host_permissions'] = GRANTED
    json.dump(manifest, open(manifest_path, 'w'))

    binary = next(path for path in ('/usr/bin/chromium-browser', '/usr/bin/chromium') if os.path.exists(path))
    driver = subprocess.Popen(['chromedriver', f'--port={DRIVER_PORT}', '--allowed-origins=*'], env=environment, stdout=open(os.path.join(home, 'chromedriver.log'), 'w'), stderr=subprocess.STDOUT)
    try:
        time.sleep(1.5)
        browser = lab.W3C(DRIVER_PORT, {'browserName': 'chrome', 'goog:chromeOptions': {
            'binary': binary,
            'excludeSwitches': ['disable-extensions'],
            'args': ['--headless=new', '--no-sandbox', '--disable-gpu', '--disable-dev-shm-usage', f'--user-data-dir={home}/.config/chromium', f'--load-extension={unpacked}',
                     '--disable-features=DisableLoadExtensionCommandLineSwitch,HttpsUpgrades,HttpsFirstBalancedModeAutoEnable', '--window-size=1280,900', '--no-first-run',
                     '--no-default-browser-check', '--disable-background-timer-throttling']}})
        print('Chromium', subprocess.run([binary, '--version'], capture_output=True, text=True).stdout.strip(), flush=True)
        time.sleep(2)
        run(browser)
    except BaseException:
        print('chromedriver log:', open(os.path.join(home, 'chromedriver.log')).read()[-3000:], flush=True)
        raise
    finally:
        driver.terminate()
        driver.wait(timeout=20)
    print(f'{lab.checks} checks, {len(lab.failures)} failed', flush=True)
    sys.exit(1 if lab.failures else 0)


def run(browser):
    extension_url = f'chrome-extension://{EXTENSION_ID}'
    main_handle = browser.current()

    def popup_opened():
        # Headless Chromium may not open the toolbar popup at all, and then the extension opens its own window
        return len(browser.handles()) > 1

    # The content script is registered a moment after the browser starts: wait for it on a page before the checks begin
    browser.go(lab.BASE + '/login.html')
    for _ in range(40):
        browser.click_element('#user')
        if lab.wait(lambda: lab.has_button(browser), 1):
            break
        browser.go(lab.BASE + '/login.html')
    lab.run(browser, extension_url, popup_opened)
    assert browser.current() == main_handle


if __name__ == '__main__':
    main()
