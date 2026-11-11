// Modified for Amethystora: Amethystora Clipboard is Clipboard Indicator v71 with its history sealed
// (registry.js, store.js, keyring.js) instead of written to ~/.cache in plain text, kept as keep-history
// says, and with nothing recorded that a password manager marks secret, and gives Amethystora UI's search a
// read-only view of the history (history(), content()), and with text actions and QR codes on each text item
// (textActions.js, Amethystora's own).

import Clutter from 'gi://Clutter';
import Cogl from 'gi://Cogl';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';
import St from 'gi://St';

import * as AnimationUtils from 'resource:///org/gnome/shell/misc/animationUtils.js';
import * as MessageTray from 'resource:///org/gnome/shell/ui/messageTray.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as ModalDialog from 'resource:///org/gnome/shell/ui/modalDialog.js';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';
import { Extension, gettext as _ } from 'resource:///org/gnome/shell/extensions/extension.js';

import { Registry, ClipboardEntry } from './registry.js';
import { copyFits } from './pictures.js';
import { DialogManager } from './confirmDialog.js';
import { PrefsFields } from './constants.js';
import { Keyboard } from './keyboard.js';
import { TextActionsDialog } from './textActions.js';

const CLIPBOARD_TYPE = St.ClipboardType.CLIPBOARD;

const INDICATOR_ICON = 'edit-paste-symbolic';

// What KeePassXC and KDE's password managers offer beside a password, with "secret", so that clipboard
// managers keep it out of their history
const PASSWORD_HINT = 'x-kde-passwordManagerHint';
// How long the app that copied may take to send what it copied (a large picture takes a moment)
const CLIPBOARD_READ_TIMEOUT = 10000;

let DELAYED_SELECTION_TIMEOUT = 750;
let MAX_REGISTRY_LENGTH       = 15;
let MAX_ENTRY_LENGTH          = 50;
let DELETE_ENABLED            = true;
let MOVE_ITEM_FIRST           = false;
let ENABLE_KEYBINDING         = true;
let PRIVATEMODE               = false;
let NOTIFY_ON_COPY            = true;
let NOTIFY_ON_CYCLE           = true;
let NOTIFY_ON_CLEAR           = true;
let CONFIRM_ON_CLEAR          = true;
let CONFIRM_ON_PINNED_DELETE  = false;
let MAX_TOPBAR_LENGTH         = 15;
let TOPBAR_DISPLAY_MODE       = 1; //0 - only icon, 1 - only clipboard content, 2 - both, 3 - neither
let CLEAR_ON_BOOT             = false;
let PASTE_ON_SELECT           = false;
let DISABLE_DOWN_ARROW        = false;
let BLINK_ICON_ON_COPY        = false;
let STRIP_TEXT                = false;
let KEEP_SELECTED_ON_CLEAR    = false;
let PASTE_BUTTON              = true;
let PINNED_ON_BOTTOM          = false;
let CACHE_IMAGES              = true;
let EXCLUDED_APPS             = [];
let CLEAR_HISTORY_ON_INTERVAL = false;
let CLEAR_HISTORY_INTERVAL    = 60;
let NEXT_HISTORY_CLEAR        = -1;
let CASE_SENSITIVE_SEARCH     = false;
let REGEX_SEARCH              = false;
let OPEN_AT_CURSOR            = false;
let SHOW_SEARCH_BAR           = true;
let SHOW_PRIVATE_MODE         = true;
let SHOW_SETTINGS_BUTTON      = true;
let SHOW_CLEAR_HISTORY_BUTTON = true;
let SHOW_DELETE_BUTTON        = true;
let SHOW_TAG_BUTTON           = true;
let SHOW_PIN_BUTTON           = true;
let SHOW_EDIT_BUTTON          = true;
let SHOW_PREVIEW_BUTTON       = true;

export default class ClipboardIndicatorExtension extends Extension {
    enable () {
        this._settings = this.getSettings();
        this.clipboardIndicator = new ClipboardIndicator({
            clipboard: St.Clipboard.get_default(),
            settings: this._settings,
            openSettings: this.openPreferences,
            uuid: this.uuid
        });

        // Amethystora UI's meters take the first places on the right of the bar, before or after this is turned on (a
        // login turns this on first, an unlock the meters): the icon goes one place past those there now, as at a login
        const meters = ['amethystora-meters-processor', 'amethystora-meters-memory', 'amethystora-meters-network']
            .filter(role => Main.panel.statusArea[role]).length;
        Main.panel.addToStatusArea('amethystoraClipboard', this.clipboardIndicator, meters + 1);
    }

    disable () {
        this.clipboardIndicator.destroy();
        this.clipboardIndicator = null;
        // Disposed here, on the main thread, rather than left to the collector: GJS sweeping a settings object while
        // dconf's worker dispatches a change to it deadlocks the shell (its toggle queue against the backend's lock)
        this._settings?.run_dispose();
        this._settings = null;
        EXCLUDED_APPS = [];
    }

    // What the menu lists, newest first, for Amethystora UI's search (clip:), which asks only when the person
    // typed clip: and reads nothing else of the extension. Plain values, frozen; nothing while private mode is
    // on, while the history is held back by a locked keyring, or before the indicator is built. Nothing is written
    // and nothing is kept here but the numbers the entries are known by while the shell runs.
    history () {
        const indicator = this.clipboardIndicator;
        if (!indicator || indicator._destroyed || PRIVATEMODE || indicator.registry?.state === 'locked')
            return [];
        return [...indicator.clipItemsRadioGroup].reverse().map(item => {
            const entry = item.entry;
            const about = aboutEntry(entry);
            const image = entry.isImage();
            if (image && about.width === undefined)
                [about.width, about.height] = pngSize(entry);
            return Object.freeze({
                id: about.id,
                kind: image ? 'image' : 'text',
                text: image ? '' : entry.getStringValue(),
                mimetype: entry.mimetype(),
                size: about.size ??= entry.asBytes().get_size(),
                width: image ? about.width : null,
                height: image ? about.height : null,
                pinned: entry.isFavorite(),
                current: !!item.currentlySelected,
                copiedAt: about.copiedAt ?? null,
            });
        });
    }

    // One entry's contents, by the id history() gave it, for the search to put back on the clipboard when the
    // person picks it; null for an id it does not know or in private mode
    content (id) {
        const indicator = this.clipboardIndicator;
        if (!indicator || indicator._destroyed || PRIVATEMODE || indicator.registry?.state === 'locked')
            return null;
        const item = indicator.clipItemsRadioGroup.find(other => ENTRY_IDS.get(other.entry)?.id === id);
        return item ? { mimetype: item.entry.mimetype(), bytes: item.entry.asBytes() } : null;
    }
}

// The numbers the search knows the entries by, and when each was seen copied in this session: in memory only
const ENTRY_IDS = new WeakMap();
let lastEntryId = 0;

function aboutEntry (entry) {
    let about = ENTRY_IDS.get(entry);
    if (!about) {
        about = { id: ++lastEntryId };
        ENTRY_IDS.set(entry, about);
    }
    return about;
}

// A PNG's width and height, from its header (IHDR, bytes 16 to 23), or nulls for any other picture
function pngSize (entry) {
    if (entry.mimetype() !== 'image/png')
        return [null, null];
    const bytes = entry.asBytes();
    if (bytes.get_size() < 24)
        return [null, null];
    const head = bytes.new_from_bytes(0, 24).get_data();
    if (String.fromCharCode(...head.slice(12, 16)) !== 'IHDR')
        return [null, null];
    const word = at => ((head[at] << 24) | (head[at + 1] << 16) | (head[at + 2] << 8) | head[at + 3]) >>> 0;
    return [word(16) || null, word(20) || null];
}

// A type name of its own, so that Clipboard Indicator installed beside it does not clash
const ClipboardIndicator = GObject.registerClass({
    GTypeName: 'AmethystoraClipboardIndicator'
}, class ClipboardIndicator extends PanelMenu.Button {
    #refreshInProgress = false;
    #_imagePreviewOverlay = null;

    destroy () {
        this._destroyed = true;
        this._keyringUnwatch?.();
        this._disconnectSettings();
        this._unbindShortcuts();
        this._disconnectSelectionListener();
        this._clearDelayedSelectionTimeout();
        this.#clearTimeouts();
        this.#closeImagePreview();
        this._textActionsDialog?.close();
        this._textActionsDialog = null;
        this._removeHistoryLabel();
        this._destroyNotifSource();
        this.dialogManager.destroy();
        this.keyboard.destroy();
        this._cursorActor.destroy();
        this._cursorActor = null;

        super.destroy();
    }

    _init (extension) {
        super._init(0.0, "ClipboardIndicator");

        this._cursorActor = new Clutter.Actor({ opacity: 0, width: 1, height: 1 });
        Main.uiGroup.add_child(this._cursorActor);

        this.menu.connect('open-state-changed', (menu, isOpen) => {
            if (!isOpen)
                this.menu.sourceActor = this;
        });

        this.extension = extension;
        this._destroyed = false;
        this.registry = new Registry(extension);
        this.keyboard = new Keyboard();
        this._settingsChangedId = null;
        this._selectionOwnerChangedId = null;
        this._historyLabel = null;
        this._buttonText = null;
        this._disableDownArrow = null;

        this._shortcutsBindingIds = [];
        this.clipItemsRadioGroup = [];

        let hbox = new St.BoxLayout({
            style_class: 'panel-status-menu-box clipboard-indicator-hbox'
        });

        this.hbox = hbox;

        this.icon = new St.Icon({
            icon_name: INDICATOR_ICON,
            style_class: 'system-status-icon clipboard-indicator-icon'
        });

        this._buttonText = new St.Label({
            text: _('Text will be here'),
            y_align: Clutter.ActorAlign.CENTER
        });

        this._buttonImgPreview = new St.Bin({
            style_class: 'clipboard-indicator-topbar-preview'
        });

        hbox.add_child(this.icon);
        hbox.add_child(this._buttonText);
        hbox.add_child(this._buttonImgPreview);
        this._downArrow = PopupMenu.arrowIcon(St.Side.BOTTOM);
        hbox.add_child(this._downArrow);
        this.add_child(hbox);
        this._createHistoryLabel();
        this._loadSettings();

        if (CLEAR_ON_BOOT) this.registry.clearCacheFolder();

        this.dialogManager = new DialogManager();
        this._buildMenu().then(() => {
            if (this._destroyed) {
                return;
            }
            this._updateTopbarLayout();
            this._setupListener();
            this._setupHistoryIntervalClearing();
            this.#watchKeyring();
        });
    }

    // The history kept from before opens once the keyring is unlocked, by whatever unlocks it
    #watchKeyring () {
        this.registry.watch(() => this.#openStored()).then(unwatch => {
            if (this._destroyed) {
                unwatch();
                return;
            }
            this._keyringUnwatch = unwatch;
            // Unlocked between the first ask and the watch
            this.#openStored();
        }).catch(e => console.warn(`Amethystora Clipboard: ${e.message}`));
    }

    #updateStoreItems () {
        if (this._destroyed || !this.lockedMenuItem) return;
        this.lockedMenuItem.visible = this.registry.state === 'locked';
        this.unsavedMenuItem.visible = this.registry.state === 'none';
    }

    async #unlockStored () {
        try {
            await this.registry.unlock();
        } catch (e) {
            console.warn(`Amethystora Clipboard: the keyring was not unlocked: ${e.message}`);
        }
        await this.#openStored();
    }

    // Once the keyring is unlocked: the history kept from before under what was copied since, and all of it
    // sealed from then on
    async #openStored () {
        // Merged once: an answer from the keyring that came after its time is merged at the next ask, and an
        // unlock while the keyring is being asked asks it again after
        if (this._mergedStored) return;
        if (this._openingStored) {
            this._askAgain = true;
            return;
        }
        this._openingStored = true;
        this._askAgain = false;
        try {
            await this.registry.open();
            if (this._destroyed || this.registry.state !== 'open') return;
            const stored = await this.registry.read();
            if (this._destroyed) return;
            this._mergedStored = true;

            const current = this.clipItemsRadioGroup.map(item => [item.entry, item.currentlySelected]);
            for (const item of [...this.clipItemsRadioGroup]) item.destroy();
            this.clipItemsRadioGroup.length = 0;
            for (const entry of stored) {
                if (!current.some(([other]) => other.equals(entry))) this._addEntry(entry, false, false);
            }
            for (const [entry, selected] of current) this._addEntry(entry, selected, false);
            this._removeOldestEntries();
            this._updateCache();
            // The pictures of what keep-history dropped
            this.registry.prune(this.clipItemsRadioGroup.map(item => item.entry));
        } catch (e) {
            console.warn(`Amethystora Clipboard: the history kept from before did not open: ${e.message}`);
        } finally {
            this._openingStored = false;
            this.#updateStoreItems();
        }
        if (this._askAgain && !this._destroyed) this.#openStored();
    }

    #updateIndicatorContent(entry) {
        if (this.preventIndicatorUpdate || (TOPBAR_DISPLAY_MODE !== 1 && TOPBAR_DISPLAY_MODE !== 2)) {
            return;
        }

        if (!entry || PRIVATEMODE) {
            this._buttonImgPreview.destroy_all_children();
            this._buttonText.set_text("...");
        } else {
            if (entry.isText()) {
                this._buttonText.set_text(this._truncate(entry.getStringValue(), MAX_TOPBAR_LENGTH));
                this._buttonImgPreview.destroy_all_children();
            }
            else if (entry.isImage()) {
                this._buttonText.set_text('');
                this._buttonImgPreview.destroy_all_children();
                this.registry.getEntryAsImage(entry).then(img => {
                    if (!img) return;
                    img.add_style_class_name('clipboard-indicator-img-preview');
                    img.y_align = Clutter.ActorAlign.CENTER;

                    // icon only renders properly in setTimeout for some arcane reason
                    this._imagePreviewTimeout = setTimeout(() => {
                        this._buttonImgPreview.set_child(img);
                    }, 0);
                });
            }
        }
    }

    _blinkIcon () {
        if (!BLINK_ICON_ON_COPY || !this.icon) {
            return;
        }

        // Set inverted colors
        this.set_style('background-color: rgba(255, 255, 255, 0.9);');
        this.icon.set_style('color: rgba(0, 0, 0, 0.9);');

        // Revert back to normal after delay
        this._blinkAnimationTimeout = setTimeout(() => {
            this._blinkAnimationTimeout = null;
            this.set_style(null);
            this.icon.set_style(null);
        }, 200);
    }

    async _buildMenu () {
        const clipHistory = await this._getCache();
        if (this._destroyed) {
            return;
        }
        let lastIdx = clipHistory.length - 1;
        let clipItemsArr = this.clipItemsRadioGroup;

        /* This create the search entry, which is add to a menuItem.
        The searchEntry is connected to the function for research.
        The menu itself is connected to some shitty hack in order to
        grab the focus of the keyboard. */
        this._entryItem = new PopupMenu.PopupBaseMenuItem({
            reactive: false,
            can_focus: false
        });
        this.searchEntry = new St.Entry({
            name: 'searchEntry',
            style_class: 'search-entry',
            can_focus: true,
            hint_text: _('Type here to search...'),
            track_hover: true,
            x_expand: true,
            y_expand: true,
            primary_icon: new St.Icon({ icon_name: 'edit-find-symbolic' })
        });

        this.searchEntry.get_clutter_text().connect(
            'text-changed',
            this._onSearchTextChanged.bind(this)
        );

        this._entryItem.add_child(this.searchEntry);

        this.menu.connect('open-state-changed', (self, open) => {
            // Asks the keyring again, without a prompt, in case it was unlocked, or made, unseen
            if (open && !this._mergedStored) this.#openStored();
            this._setFocusOnOpenTimeout = setTimeout(() => {
                if (!open) return;

                if (this._focusItemOnOpen) {
                    const item = this._focusItemOnOpen;
                    this._focusItemOnOpen = null;
                    global.stage.set_key_focus(item.actor);
                } else if (SHOW_SEARCH_BAR && !PRIVATEMODE && this.clipItemsRadioGroup.length > 0) {
                    this.searchEntry.set_text('');
                    global.stage.set_key_focus(this.searchEntry);
                } else if (!PRIVATEMODE && this.clipItemsRadioGroup.length > 0) {
                    const currentItem = this._getCurrentlySelectedItem();
                    if (currentItem) global.stage.set_key_focus(currentItem.actor);
                } else if (SHOW_PRIVATE_MODE && this.privateModeMenuItem) {
                    global.stage.set_key_focus(this.privateModeMenuItem.actor);
                }
            }, 50);
        });

        // Create menu sections for items
        // Favorites
        this.favoritesSection = new PopupMenu.PopupMenuSection();

        this.scrollViewFavoritesMenuSection = new PopupMenu.PopupMenuSection();
        this.favoritesScrollView = new St.ScrollView({
            style_class: 'ci-history-menu-section',
            overlay_scrollbars: true
        });
        this.favoritesScrollView.add_child(this.favoritesSection.actor);

        this.scrollViewFavoritesMenuSection.actor.add_child(this.favoritesScrollView);
        this.favoritesSeparator = new PopupMenu.PopupSeparatorMenuItem();

        // History
        this.historySection = new PopupMenu.PopupMenuSection();

        this.scrollViewMenuSection = new PopupMenu.PopupMenuSection();
        this.historyScrollView = new St.ScrollView({
            style_class: 'ci-main-menu-section ci-history-menu-section',
            overlay_scrollbars: true
        });
        this.historyScrollView.add_child(this.historySection.actor);

        this.scrollViewMenuSection.actor.add_child(this.historyScrollView);

        // Add separator
        this.historySeparator = new PopupMenu.PopupSeparatorMenuItem();

        // Add sections ordered according to settings
        if (PINNED_ON_BOTTOM) {
            this.menu.addMenuItem(this.scrollViewMenuSection);
            this.menu.addMenuItem(this.scrollViewFavoritesMenuSection);
        }
        else {
            this.menu.addMenuItem(this.scrollViewFavoritesMenuSection);
            this.menu.addMenuItem(this.scrollViewMenuSection);
        }

        // Private mode switch
        this.privateModeMenuItem = new PopupMenu.PopupSwitchMenuItem(
            _("Private mode"), PRIVATEMODE, { reactive: true });
        this.privateModeMenuItem.connect('toggled',
            this._onPrivateModeSwitch.bind(this));
        this.privateModeMenuItem.insert_child_at_index(
            new St.Icon({
                icon_name: 'security-medium-symbolic',
                style_class: 'clipboard-menu-icon',
                y_align: Clutter.ActorAlign.CENTER
            }),
            0
        );
        this.menu.addMenuItem(this.privateModeMenuItem);

        // Add 'Clear' button which removes all items from cache
        this.clearMenuItem = new PopupMenu.PopupMenuItem(_('Clear history'));
        this.clearMenuItem.insert_child_at_index(
            new St.Icon({
                icon_name: 'user-trash-symbolic',
                style_class: 'clipboard-menu-icon',
                y_align: Clutter.ActorAlign.CENTER
            }),
            0
        );

        let timerBox = new St.BoxLayout({
            x_align: Clutter.ActorAlign.END,
            x_expand: true
        });

        this.timerLabel = new St.Label({
            text: '',
            style: 'font-family: monospace;',
            x_align: Clutter.ActorAlign.END,
            x_expand: true
        });

        this.resetTimerButton = new St.Button({
            style_class: 'ci-action-btn',
            can_focus: true,
            accessible_name: _('Reset Timer'),
            child: new St.Icon({
                icon_name: 'view-refresh-symbolic',
                style_class: 'system-status-icon',
                icon_size: 14
            }),
            x_align: Clutter.ActorAlign.END,
            y_align: Clutter.ActorAlign.CENTER,
        });

        this.resetTimerButton.connect('clicked', () => {
            this._scheduleNextHistoryClear();
        });

        timerBox.add_child(this.timerLabel);
        timerBox.add_child(this.resetTimerButton);
        this.clearMenuItem.add_child(timerBox);

        this.clearMenuItem.connect('activate', this._removeAll.bind(this));

        // Add 'Settings' menu item to open settings
        this.settingsMenuItem = new PopupMenu.PopupMenuItem(_('Settings'));
        this.settingsMenuItem.insert_child_at_index(
            new St.Icon({
                icon_name: 'preferences-system-symbolic',
                style_class: 'clipboard-menu-icon',
                y_align: Clutter.ActorAlign.CENTER
            }),
            0
        );
        this.settingsMenuItem.connect('activate', this._openSettings.bind(this));

        // While the keyring is locked, a row that asks for it to be unlocked; with no keyring at all, one
        // that says nothing is kept
        this.lockedMenuItem = new PopupMenu.PopupMenuItem(_('Unlock to see earlier items'));
        this.lockedMenuItem.insert_child_at_index(
            new St.Icon({
                icon_name: 'changes-prevent-symbolic',
                style_class: 'clipboard-menu-icon',
                y_align: Clutter.ActorAlign.CENTER
            }),
            0
        );
        this.lockedMenuItem.connect('activate', () => this.#unlockStored());
        this.menu.addMenuItem(this.lockedMenuItem, 0);
        this.unsavedMenuItem = new PopupMenu.PopupMenuItem(
            _('Not saved: there is no keyring to keep it in'), { reactive: false });
        this.menu.addMenuItem(this.unsavedMenuItem, 0);
        this.#updateStoreItems();

        // Empty state section
        this.emptyStateSection = new St.BoxLayout({
            style_class: 'clipboard-indicator-empty-state',
            // GNOME 51 has no vertical property: orientation, as 50 takes too
            orientation: Clutter.Orientation.VERTICAL
        });
        this.emptyStateSection.add_child(new St.Icon({
            icon_name: INDICATOR_ICON,
            style_class: 'system-status-icon clipboard-indicator-icon',
            x_align: Clutter.ActorAlign.CENTER
        }));
        this.emptyStateSection.add_child(new St.Label({
            text: _('Clipboard is empty'),
            x_align: Clutter.ActorAlign.CENTER
        }));

        // Add cached items
        clipHistory.forEach(entry => this._addEntry(entry));

        if (lastIdx >= 0) {
            this._selectMenuItem(clipItemsArr[lastIdx]);
        }

        this.#showElements();
    }

    // Whether the menu holds CHILD: GNOME 51's Clutter takes no undefined actor, which one not made yet is
    #holds(child) {
        return !!child && this.menu.box.contains(child);
    }

    #hideElements() {
        if (this._destroyed) {
            return;
        }
        if (this.#holds(this._entryItem)) this.menu.box.remove_child(this._entryItem);
        if (this.#holds(this.favoritesSeparator)) this.menu.box.remove_child(this.favoritesSeparator);
        if (this.#holds(this.historySeparator)) this.menu.box.remove_child(this.historySeparator);
        if (this.clearMenuItem?.actor && this.#holds(this.clearMenuItem.actor))
            this.menu.box.remove_child(this.clearMenuItem.actor);
        if (this.settingsMenuItem?.actor && this.#holds(this.settingsMenuItem.actor))
            this.menu.box.remove_child(this.settingsMenuItem.actor);
        if (this.#holds(this.emptyStateSection)) this.menu.box.remove_child(this.emptyStateSection);
    }

    #showElements() {
        if (this._destroyed) {
            return;
        }

        // Remove empty-state if items exist
        if (this.clipItemsRadioGroup.length > 0 &&
            this.#holds(this.emptyStateSection)) {
            this.menu.box.remove_child(this.emptyStateSection);
        }

        // Search bar
        if (SHOW_SEARCH_BAR && !PRIVATEMODE) {
            if (!this.#holds(this._entryItem))
                this.menu.box.insert_child_at_index(this._entryItem, 0);
        } else {
            if (this.#holds(this._entryItem))
                this.menu.box.remove_child(this._entryItem);
        }

        // Keep the private-mode switch in place; only gate its visibility
        if (this.privateModeMenuItem?.actor) {
            this.privateModeMenuItem.actor.visible = SHOW_PRIVATE_MODE;
        }

        // Favorites separator (only when there are favorites and any items at all)
        if (this.clipItemsRadioGroup.length > 0) {
            if (this.favoritesSection._getMenuItems().length > 0 && !PRIVATEMODE) {
                if (this.#holds(this.favoritesSeparator) === false) {
                    this.menu.box.insert_child_above(this.favoritesSeparator, this.scrollViewFavoritesMenuSection.actor);
                }
            }
            else if (this.#holds(this.favoritesSeparator) === true) {
                this.menu.box.remove_child(this.favoritesSeparator);
            }
        }

        // History separator (between history and toggled buttons)
        if (this.clipItemsRadioGroup.length > 0 &&
            this.historySection._getMenuItems().length > 0 && !PRIVATEMODE &&
            (SHOW_PRIVATE_MODE || SHOW_SETTINGS_BUTTON || SHOW_CLEAR_HISTORY_BUTTON)) {
            if (!this.#holds(this.historySeparator))
                this.menu.box.insert_child_above(this.historySeparator, this.scrollViewMenuSection.actor);
        } else if (this.#holds(this.historySeparator)) {
            this.menu.box.remove_child(this.historySeparator);
        }

        // If no items, render empty state and (if toggled on) only show Private/Settings
        if (this.clipItemsRadioGroup.length === 0) {
            if (!this.#holds(this.emptyStateSection))
                this.#renderEmptyState();
            // Re-append toggled buttons after the empty state
            if (this.#holds(this.settingsMenuItem?.actor))
                this.menu.box.remove_child(this.settingsMenuItem.actor);

            let index = this.menu.box.get_n_children(); // append after empty state
            if (SHOW_SETTINGS_BUTTON && this.settingsMenuItem)
                this.menu.box.insert_child_at_index(this.settingsMenuItem.actor, index++);
            return;
        }

        // Re-append toggled buttons at end in fixed order
        if (this.#holds(this.settingsMenuItem?.actor))
            this.menu.box.remove_child(this.settingsMenuItem.actor);
        if (this.#holds(this.clearMenuItem?.actor))
            this.menu.box.remove_child(this.clearMenuItem.actor);

        let index = this.menu.box.get_n_children(); // append
        if (SHOW_SETTINGS_BUTTON && this.settingsMenuItem)
            this.menu.box.insert_child_at_index(this.settingsMenuItem.actor, index++);
        if (SHOW_CLEAR_HISTORY_BUTTON && this.clearMenuItem && !PRIVATEMODE)
            this.menu.box.insert_child_at_index(this.clearMenuItem.actor, index++);
    }

    #renderEmptyState () {
        if (this._destroyed) {
            return;
        }
        this.#hideElements();
        this.menu.box.insert_child_at_index(this.emptyStateSection, 0);
    }

    /* When text change, this function will check, for each item of the
    historySection and favoritesSestion, if it should be visible or not (based on words contained
    in the clipContents attribute of the item). It doesn't destroy or create
    items. It the entry is empty, the section is restored with all items
    set as visible. */
    _onSearchTextChanged () {

        // Text to be searched converted to lowercase if search is case insensitive
        let searchedText = this.searchEntry.get_text();
        if (!CASE_SENSITIVE_SEARCH) searchedText = searchedText.toLowerCase();

        if(searchedText === '') {
            this._getAllIMenuItems().forEach(function(mItem){
                mItem.actor.visible = true;
            });
        }
        else {
        this._getAllIMenuItems().forEach((mItem) => {
                let text = mItem.clipContents;
                let tag = mItem.entry.getTag() || '';
                if (!CASE_SENSITIVE_SEARCH) {
                    text = text.toLowerCase();
                    tag = tag.toLowerCase();
                }

                let isMatching = false;
                if (REGEX_SEARCH) {
                    const flags = 'm' + (CASE_SENSITIVE_SEARCH ? '' : 'i');
                    const re = new RegExp(searchedText, flags);
                    isMatching = re.test(text) || re.test(tag);
                } else {
                    isMatching = text.includes(searchedText) || tag.includes(searchedText);
                }
                mItem.actor.visible = isMatching;
            });
        }
    }

    _truncate (string, length) {
        let shortened = string.replace(/\s+/g, ' ');

        let chars = [...shortened]
        if (chars.length > length)
            shortened = chars.slice(0, length - 1).join('') + '...';

        return shortened;
    }

    _setEntryLabel (menuItem) {
        const { entry } = menuItem;
        if (entry.isText()) {
            menuItem.label.set_text(this._truncate(entry.getStringValue(), MAX_ENTRY_LENGTH));
        }
        else if (entry.isImage()) {
            this.registry.getEntryAsImage(entry).then(img => {
                if (!img) return;
                img.add_style_class_name('clipboard-menu-img-preview');
                if (menuItem.previewImage) {
                    menuItem.remove_child(menuItem.previewImage);
                }
                menuItem.previewImage = img;
                menuItem.insert_child_below(img, menuItem.label);
            });
        }
    }

    _findNextMenuItem (currentMenutItem) {
        let currentIndex = this.clipItemsRadioGroup.indexOf(currentMenutItem);

        // for only one item
        if(this.clipItemsRadioGroup.length === 1) {
            return null;
        }

        // when focus is in middle of the displayed list
        for (let i = currentIndex - 1; i >= 0; i--) {
            let menuItem = this.clipItemsRadioGroup[i];
            if (menuItem.actor.visible) {
                return menuItem;
            }
        }

        // when focus is at the last element of the displayed list
        let beforeMenuItem = this.clipItemsRadioGroup[currentIndex + 1];
        if(beforeMenuItem.actor.visible){
          return beforeMenuItem;
        }

        return null;
    }

    #selectNextMenuItem (menuItem) {
        let nextMenuItem = this._findNextMenuItem(menuItem);

        if (nextMenuItem) {
            nextMenuItem.actor.grab_key_focus();
        } else if (this.privateModeMenuItem?.actor) {
            this.privateModeMenuItem.actor.grab_key_focus();
        }
    }

    _addEntry (entry, autoSelect, autoSetClip) {
        let menuItem = new PopupMenu.PopupMenuItem('');

        menuItem.menu = this.menu;
        menuItem.entry = entry;
        menuItem.clipContents = entry.getStringValue();
        menuItem.radioGroup = this.clipItemsRadioGroup;

        // CLICK fix for Paste on Select: clicking behaves like Enter
        menuItem.connect('activate', () => {
            if (PASTE_ON_SELECT) {
                this.#pasteItem(menuItem);
                this._onMenuItemSelectedAndMenuClose(menuItem, false);
            } else {
                this._onMenuItemSelectedAndMenuClose(menuItem, true);
            }
        });

        menuItem.connect('key-focus-in', () => {
            const viewToScroll = menuItem.entry.isFavorite() ?
                this.favoritesScrollView : this.historyScrollView;
            AnimationUtils.ensureActorVisibleInScrollView(viewToScroll, menuItem);
        });
        menuItem.actor.connect('key-press-event', (actor, event) => {
            switch (event.get_key_symbol()) {
                case Clutter.KEY_Delete:
                    if (menuItem.entry.isFavorite()) {
                        if (CONFIRM_ON_PINNED_DELETE) {
                            this._confirmRemovePinnedEntry(menuItem, true);
                        } else {
                            this.#selectNextMenuItem(menuItem);
                            this._removeEntry(menuItem, 'delete');
                        }
                    } else {
                        this.#selectNextMenuItem(menuItem);
                        this._removeEntry(menuItem, 'delete');
                    }
                    return Clutter.EVENT_STOP;
                case Clutter.KEY_p:
                    this.#selectNextMenuItem(menuItem);
                    this._favoriteToggle(menuItem);
                    return Clutter.EVENT_STOP;
                case Clutter.KEY_v:
                    this.#pasteItem(menuItem);
                    return Clutter.EVENT_STOP;
                case Clutter.KEY_h:
                    if (entry.isImage()) {
                        this.#showImagePreview(entry, () => {
                            this._focusItemOnOpen = menuItem;
                            this.menu.open();
                        });
                        return Clutter.EVENT_STOP;
                    }
                    break;
                case Clutter.KEY_e:
                    if (entry.isText()) {
                        this.#showEditDialog(menuItem, true);
                        return Clutter.EVENT_STOP;
                    }
                    break;
                case Clutter.KEY_t:
                    this.#showTagDialog(menuItem, true);
                    return Clutter.EVENT_STOP;
                case Clutter.KEY_a:
                    if (entry.isText()) {
                        this.#showTextActions(menuItem, true);
                        return Clutter.EVENT_STOP;
                    }
                    break;
                case Clutter.KEY_KP_Enter:
                case Clutter.KEY_Return:
                    if (PASTE_ON_SELECT) {
                        this.#pasteItem(menuItem);
                        this._onMenuItemSelectedAndMenuClose(menuItem, false);
                    } else {
                        this._onMenuItemSelectedAndMenuClose(menuItem, true);
                    }
                    return Clutter.EVENT_STOP;
            }
            return Clutter.EVENT_PROPAGATE;
        });

        this._setEntryLabel(menuItem);
        this.clipItemsRadioGroup.push(menuItem);

        if (entry.getTag()) {
            menuItem.tagLabel = new St.Label({
                text: entry.getTag(),
                style_class: 'ci-tag-label',
                y_align: Clutter.ActorAlign.CENTER,
            });
            menuItem.actor.add_child(menuItem.tagLabel);
        }

        menuItem.actionsSpacer = new St.Widget({
            x_expand: true,
        });
        menuItem.actor.add_child(menuItem.actionsSpacer);

        // Image preview button
        if (entry.isImage()) {
            menuItem.imagePreviewBtn = new St.Button({
                style_class: 'ci-action-btn',
                can_focus: true,
                accessible_name: _('Preview Image'),
                child: new St.Icon({
                    icon_name: 'image-x-generic-symbolic',
                    style_class: 'system-status-icon'
                }),
                visible: SHOW_PREVIEW_BUTTON,
                x_expand: false,
                y_expand: true,
            });
            menuItem.imagePreviewBtn.connect('clicked', () => this.#showImagePreview(entry));
            menuItem.actor.add_child(menuItem.imagePreviewBtn);
        }

        // Edit button (text entries only)
        if (entry.isText()) {
            menuItem.editBtn = new St.Button({
                style_class: 'ci-action-btn',
                can_focus: true,
                accessible_name: _('Edit'),
                child: new St.Icon({
                    icon_name: 'document-edit-symbolic',
                    style_class: 'system-status-icon',
                }),
                visible: SHOW_EDIT_BUTTON,
                x_expand: false,
                y_expand: true,
            });
            menuItem.editBtn.connect('clicked', () => this.#showEditDialog(menuItem));
            menuItem.actor.add_child(menuItem.editBtn);

            // Text actions and the QR code (textActions.js)
            menuItem.actionsBtn = new St.Button({
                style_class: 'ci-action-btn',
                can_focus: true,
                accessible_name: _('Text actions'),
                child: new St.Icon({
                    icon_name: 'view-more-symbolic',
                    style_class: 'system-status-icon',
                }),
                x_expand: false,
                y_expand: true,
            });
            menuItem.actionsBtn.connect('clicked', () => this.#showTextActions(menuItem));
            menuItem.actor.add_child(menuItem.actionsBtn);
        }

        // Favorite button
        let iconfav = new St.Icon({
            icon_name: 'view-pin-symbolic',
            style_class: 'system-status-icon'
        });

        let icofavBtn = new St.Button({
            style_class: 'ci-pin-btn ci-action-btn',
            can_focus: true,
            child: iconfav,
            visible: SHOW_PIN_BUTTON,
            x_expand: false,
            y_expand: true
        });

        menuItem.actor.add_child(icofavBtn);
        menuItem.icofavBtn = icofavBtn;
        menuItem.favoritePressId = icofavBtn.connect('clicked',
            () => this._favoriteToggle(menuItem)
        );

        // Paste button
        menuItem.pasteBtn = new St.Button({
            style_class: 'ci-action-btn',
            can_focus: true,
            accessible_name: _('Paste'),
            child: new St.Icon({
                icon_name: 'edit-paste-symbolic',
                style_class: 'system-status-icon'
            }),
            x_expand: false,
            y_expand: true,
            visible: PASTE_BUTTON
        });

        menuItem.pasteBtn.connect('clicked',
            () => this.#pasteItem(menuItem)
        );

        menuItem.actor.add_child(menuItem.pasteBtn);

        // Tag button
        const tagIcon = new St.Icon({
            icon_name: 'user-bookmarks-symbolic',
            style_class: 'system-status-icon',
        });

        menuItem.tagBtn = new St.Button({
            style_class: 'ci-action-btn',
            can_focus: true,
            child: tagIcon,
            visible: SHOW_TAG_BUTTON,
            x_expand: false,
            y_expand: true,
        });
        menuItem.tagBtn.connect('clicked', () => this.#showTagDialog(menuItem));
        menuItem.actor.add_child(menuItem.tagBtn);

        // Delete button
        let icon = new St.Icon({
            icon_name: 'edit-delete-symbolic', //'mail-attachment-symbolic',
            style_class: 'system-status-icon'
        });

        let icoBtn = new St.Button({
            style_class: 'ci-action-btn',
            can_focus: true,
            child: icon,
            visible: SHOW_DELETE_BUTTON,
            x_expand: false,
            y_expand: true
        });

        menuItem.actor.add_child(icoBtn);
        menuItem.icoBtn = icoBtn;
        menuItem.deletePressId = icoBtn.connect('clicked',
            () => menuItem.entry.isFavorite()
                ? (CONFIRM_ON_PINNED_DELETE
                    ? this._confirmRemovePinnedEntry(menuItem)
                    : this._removeEntry(menuItem, 'delete'))
                : this._removeEntry(menuItem, 'delete')
        );

        if (entry.isFavorite()) {
            this.favoritesSection.addMenuItem(menuItem, 0);
        } else {
            this.historySection.addMenuItem(menuItem, 0);
        }

        if (autoSelect === true) {
            this._selectMenuItem(menuItem, autoSetClip);
        }
        else {
            menuItem.setOrnament(PopupMenu.Ornament.DOT);
            if (menuItem._ornamentIcon) menuItem._ornamentIcon.opacity = 0;
        }

        this.#showElements();
    }

    _favoriteToggle (menuItem) {
        menuItem.entry.favorite = menuItem.entry.isFavorite() ? false : true;
        this._moveItemFirst(menuItem);
        this._updateCache();
        this.#showElements();
    }

    _confirmRemovePinnedEntry (menuItem, selectNext = false) {
        const title = _("Delete pinned item?");
        const message = _("Are you sure you want to delete this pinned item?");
        const sub_message = _("This operation cannot be undone.");

        this.dialogManager.open(title, message, sub_message, _("Delete"), _("Cancel"), () => {
            if (selectNext) this.#selectNextMenuItem(menuItem);
            this._removeEntry(menuItem, 'delete');
        });
    }

    _confirmRemoveAll () {
        const title = _("Clear all?");
        const message = _("Are you sure you want to delete all clipboard items?");
        const sub_message = _("This operation cannot be undone.");

        this.dialogManager.open(title, message, sub_message, _("Clear"), _("Cancel"), () => {
            this._clearHistory();
        }
      );
    }

    _clearHistory (invokedAutomatically = false) {
        // Don't remove pinned items
        this.historySection._getMenuItems().forEach(mItem => {
            if (KEEP_SELECTED_ON_CLEAR === false || !mItem.currentlySelected) {
                this._removeEntry(mItem, 'delete');
            }
        });

        if (NOTIFY_ON_CLEAR) {
            const message = invokedAutomatically
                ? _("Clipboard history cleared automatically")
                : _("Clipboard history cleared");
            this._showNotification(message);
        }
    }

    _removeAll () {
        if (PRIVATEMODE) return;

        if (CONFIRM_ON_CLEAR) {
            this._confirmRemoveAll();
        } else {
            this._clearHistory();
        }
    }

    _removeEntry (menuItem, event) {
        let itemIdx = this.clipItemsRadioGroup.indexOf(menuItem);

        if(event === 'delete' && menuItem.currentlySelected) {
            this.#clearClipboard();
        }

        menuItem.destroy();
        this.clipItemsRadioGroup.splice(itemIdx,1);

        if (menuItem.entry.isImage()) {
            this.registry.deleteEntryFile(menuItem.entry);
        }

        this._updateCache();
        this.#showElements();
    }

    _removeOldestEntries () {
        let clipItemsRadioGroupNoFavorite = this.clipItemsRadioGroup.filter(
            item => item.entry.isFavorite() === false);

        const origSize = clipItemsRadioGroupNoFavorite.length;

        while (clipItemsRadioGroupNoFavorite.length > MAX_REGISTRY_LENGTH) {
            let oldestNoFavorite = clipItemsRadioGroupNoFavorite.shift();
            this._removeEntry(oldestNoFavorite);

            clipItemsRadioGroupNoFavorite = this.clipItemsRadioGroup.filter(
                item => item.entry.isFavorite() === false);
        }

        if (clipItemsRadioGroupNoFavorite.length < origSize) {
            this._updateCache();
        }
    }

    _onMenuItemSelected (menuItem, autoSet) {
        for (let otherMenuItem of menuItem.radioGroup) {
            let clipContents = menuItem.clipContents;

            if (otherMenuItem === menuItem && clipContents) {
                menuItem.setOrnament(PopupMenu.Ornament.DOT);
                if (menuItem._ornamentIcon) menuItem._ornamentIcon.opacity = 255;
                menuItem.currentlySelected = true;
                if (autoSet !== false)
                    this.#updateClipboard(menuItem.entry);
            }
            else {
                otherMenuItem.setOrnament(PopupMenu.Ornament.DOT);
                if (otherMenuItem._ornamentIcon) otherMenuItem._ornamentIcon.opacity = 0;
                otherMenuItem.currentlySelected = false;
            }
        }
    }

    _selectMenuItem (menuItem, autoSet) {
        this._onMenuItemSelected(menuItem, autoSet);
        this.#updateIndicatorContent(menuItem.entry);
    }

    _onMenuItemSelectedAndMenuClose (menuItem, autoSet) {
        for (let otherMenuItem of menuItem.radioGroup) {
            let clipContents = menuItem.clipContents;

            if (menuItem === otherMenuItem && clipContents) {
                menuItem.setOrnament(PopupMenu.Ornament.DOT);
                if (menuItem._ornamentIcon) menuItem._ornamentIcon.opacity = 255;
                menuItem.currentlySelected = true;
                if (autoSet !== false)
                    this.#updateClipboard(menuItem.entry);
            }
            else {
                otherMenuItem.setOrnament(PopupMenu.Ornament.DOT);
                if (otherMenuItem._ornamentIcon) otherMenuItem._ornamentIcon.opacity = 0;
                otherMenuItem.currentlySelected = false;
            }
        }

        // Ensure MOVE_ITEM_FIRST also applies when PASTE_ON_SELECT fast-path skips _refreshIndicator()
        if (PASTE_ON_SELECT && MOVE_ITEM_FIRST && !menuItem.entry.isFavorite()) {
            this._moveItemFirst(menuItem);
        }

        menuItem.menu.close();
    }

    async _getCache () {
        await this.registry.open();
        this._mergedStored = this.registry.state === 'open';
        return this.registry.read({ prune: true });
    }

    #addToCache (entry) {
        const entries = this.clipItemsRadioGroup
            .map(menuItem => menuItem.entry)
            .concat([entry]);
        this.registry.write(entries).catch(e => console.warn(`Amethystora Clipboard: ${e.message}`));
    }

    _updateCache () {
        const entries = this.clipItemsRadioGroup
            .map(menuItem => menuItem.entry);

        this.registry.write(entries).catch(e => console.warn(`Amethystora Clipboard: ${e.message}`));
    }

    async _onSelectionChange (selection, selectionType, selectionSource) {
        if (selectionType === Meta.SelectionType.SELECTION_CLIPBOARD) {
            this._refreshIndicator();
        }
    }

    async _refreshIndicator () {
        if (PRIVATEMODE || this._destroyed) return; // Private mode, do not.

        const focussedWindow = Shell.Global.get().display.focusWindow;
        const wmClass = focussedWindow?.get_wm_class();
        // A Flatpak's id too, which is what its window class is not always
        const appId = focussedWindow?.get_sandboxed_app_id();

        if (wmClass && EXCLUDED_APPS.includes(wmClass)) return; // Excluded app, do not.
        if (appId && EXCLUDED_APPS.includes(appId)) return;

        if (this.#refreshInProgress) return;
        this.#refreshInProgress = true;

        try {
            const result = await this.#getClipboardContent();
            if (this._destroyed) {
                return;
            }

            if (result) {
                for (let menuItem of this.clipItemsRadioGroup) {
                    if (menuItem.entry.equals(result)) {
                        aboutEntry(menuItem.entry).copiedAt = GLib.DateTime.new_now_local();
                        this._selectMenuItem(menuItem, false);

                        if (!menuItem.entry.isFavorite() && MOVE_ITEM_FIRST) {
                            this._moveItemFirst(menuItem);
                        }

                        return;
                    }
                }

                this.#addToCache(result);
                aboutEntry(result).copiedAt = GLib.DateTime.new_now_local();
                this._addEntry(result, true, false);
                this._removeOldestEntries();
                if (NOTIFY_ON_COPY) {
                    this._showNotification(_("Copied to clipboard"), notif => {
                        notif.addAction(_('Cancel'), this._cancelNotification);
                    });
                }
                this._blinkIcon();
            }
        }
        catch (e) {
            console.error('Amethystora Clipboard: Failed to refresh indicator');
            console.error(e);
        }
        finally {
            this.#refreshInProgress = false;
        }
    }

    _moveItemFirst (item) {
        this._removeEntry(item);
        this._addEntry(item.entry, item.currentlySelected, false);
        this._updateCache();
    }

    _findItem (text) {
        return this.clipItemsRadioGroup.filter(
            item => item.clipContents === text)[0];
    }

    _getCurrentlySelectedItem () {
        return this.clipItemsRadioGroup.find(item => item.currentlySelected);
    }

    _getAllIMenuItems () {
        return this.historySection._getMenuItems().concat(this.favoritesSection._getMenuItems());
    }

    _setupListener () {
        const metaDisplay = Shell.Global.get().get_display();
        const selection = metaDisplay.get_selection();
        this._setupSelectionTracking(selection);
    }

    _setupSelectionTracking (selection) {
        this.selection = selection;
        this._selectionOwnerChangedId = selection.connect('owner-changed', (selection, selectionType, selectionSource) => {
            this._onSelectionChange(selection, selectionType, selectionSource);
        });
    }

    _setupHistoryIntervalClearing() {
        this._fetchSettings();

        if (this._intervalSettingChangedId) {
            this.extension.settings.disconnect(this._intervalSettingChangedId);
            this._intervalSettingChangedId = null;
        }
        if (this._intervalToggleChangedId) {
            this.extension.settings.disconnect(this._intervalToggleChangedId);
            this._intervalToggleChangedId = null;
        }
        if (this._historyClearTimeoutId) {
            clearTimeout(this._historyClearTimeoutId);
            this._historyClearTimeoutId = null;
        }

        this._intervalSettingChangedId = this.extension.settings.connect(
            `changed::${PrefsFields.CLEAR_HISTORY_INTERVAL}`,
            this._onHistoryIntervalClearSettingsChanged.bind(this)
        );
        this._intervalToggleChangedId = this.extension.settings.connect(
            `changed::${PrefsFields.CLEAR_HISTORY_ON_INTERVAL}`,
            this._onHistoryIntervalClearSettingsChanged.bind(this)
        );

        if (!CLEAR_HISTORY_ON_INTERVAL) {
            this._updateIntervalTimer();
            return;
        }

        const currentTime = Math.ceil(new Date().getTime() / 1000);

        if (NEXT_HISTORY_CLEAR === -1) { //new timer
            this._scheduleNextHistoryClear();
        }
        else if (NEXT_HISTORY_CLEAR < currentTime) { //timer expired
            this._clearHistory(true);
            this._scheduleNextHistoryClear();
        }
        else { //timer already set, but not expired
            // Clean up existing timers before reassigning
            if (this._historyClearTimeoutId) {
                clearTimeout(this._historyClearTimeoutId);
                this._historyClearTimeoutId = null;
            }
            if (this._timerIntervalId) {
                clearInterval(this._timerIntervalId);
                this._timerIntervalId = null;
            }
            
            const timeoutMs = (NEXT_HISTORY_CLEAR - currentTime) * 1000;
            this._historyClearTimeoutId = setTimeout(() => {
                this._clearHistory(true);
                this._scheduleNextHistoryClear();
            }, timeoutMs);
            this._timerIntervalId = setInterval(() => {
                this._updateIntervalTimer();
            }, 1000);
        }
    }

    _onHistoryIntervalClearSettingsChanged(_settings, key) {
        this._fetchSettings();
        if (key === PrefsFields.CLEAR_HISTORY_INTERVAL) {
            this._scheduleNextHistoryClear();
        }
        else if (key === PrefsFields.CLEAR_HISTORY_ON_INTERVAL) {
            if (CLEAR_HISTORY_ON_INTERVAL) {
                this._resetHistoryClearTimer();
                this._setupHistoryIntervalClearing();
            } else {
                this._resetHistoryClearTimer();
            }
        }
    }

    _scheduleNextHistoryClear() {
        this._fetchSettings();

        clearInterval(this._timerIntervalId);
        if (this._historyClearTimeoutId) {
            clearTimeout(this._historyClearTimeoutId);
            this._historyClearTimeoutId = null;
        }

        if(!CLEAR_HISTORY_ON_INTERVAL) {
            this._resetHistoryClearTimer();
            return;
        }

        const currentTime = Math.ceil(new Date().getTime() / 1000);
        NEXT_HISTORY_CLEAR = currentTime + CLEAR_HISTORY_INTERVAL * 60;
        const timeoutMs = (NEXT_HISTORY_CLEAR - currentTime) * 1000;

        this.extension.settings.set_int(PrefsFields.NEXT_HISTORY_CLEAR, NEXT_HISTORY_CLEAR);

        this._updateIntervalTimer();
        this._timerIntervalId = setInterval(() => {
            this._updateIntervalTimer();
        }, 1000);

        this._historyClearTimeoutId = setTimeout(() => {
            this._clearHistory(true);
            this._scheduleNextHistoryClear();
        }, timeoutMs);
    }

    _resetHistoryClearTimer() {
        //basically just reset and stop the timer
        if (this._historyClearTimeoutId) {
            clearTimeout(this._historyClearTimeoutId);
            this._historyClearTimeoutId = null;
        }
        clearInterval(this._timerIntervalId);
        this._timerIntervalId = null;
        this._updateIntervalTimer();
        this.extension.settings.set_int(PrefsFields.NEXT_HISTORY_CLEAR, -1);
    }

    _updateIntervalTimer() {
        this._fetchSettings();
        this.resetTimerButton.visible = CLEAR_HISTORY_ON_INTERVAL;
        this.timerLabel.visible = CLEAR_HISTORY_ON_INTERVAL;
        if (!CLEAR_HISTORY_ON_INTERVAL) return;


        let currentTime = Math.ceil(new Date().getTime() / 1000);
        let timeLeft = NEXT_HISTORY_CLEAR - currentTime;

        if (timeLeft <= 0) {
            this.timerLabel.set_text('');
            return;
        }

        let hours = Math.floor(timeLeft / 3600);
        let minutes = Math.floor((timeLeft % 3600) / 60);
        let seconds = Math.floor(timeLeft % 60);

        let formattedTime = '';
        if (hours > 0) {
            formattedTime += `${hours}h `;
        }
        if (minutes > 0) {
            formattedTime += `${minutes}m `;
        }
        formattedTime += `${seconds}s`;
        this.timerLabel.set_text(formattedTime);
    }

    _openSettings () {
        this.extension.openSettings();
        this.menu.close();
    }

    _initNotifSource () {
        if (!this._notifSource) {
            this._notifSource = new MessageTray.Source({
                title: 'Amethystora Clipboard',
                'icon-name': INDICATOR_ICON
            });

            this._notifSource.connect('destroy', () => {
                this._notifSource = null;
            });

            Main.messageTray.add(this._notifSource);
        }
    }

    _destroyNotifSource () {
        if (this._notifSource) {
            this._notifSource.destroy();
            this._notifSource = null;
        }
    }

    _cancelNotification () {
        if (this.clipItemsRadioGroup.length >= 2) {
            let clipSecond = this.clipItemsRadioGroup.length - 2;
            let previousClip = this.clipItemsRadioGroup[clipSecond];
            this.#updateClipboard(previousClip.entry);
            previousClip.setOrnament(PopupMenu.Ornament.DOT);
            previousClip.icoBtn.visible = false;
            previousClip.currentlySelected = true;
        } else {
            this.#clearClipboard();
        }
        let clipFirst = this.clipItemsRadioGroup.length - 1;
        this._removeEntry(this.clipItemsRadioGroup[clipFirst]);
    }

    _showNotification (message, transformFn) {
        const dndOn = () =>
            !Main.panel.statusArea.dateMenu._indicator._settings.get_boolean(
                'show-banners',
            );
        if (PRIVATEMODE || dndOn()) {
            return;
        }

        let notification = null;

        this._initNotifSource();

        if (this._notifSource.count === 0) {
            notification = new MessageTray.Notification({
                source: this._notifSource,
                body: message,
                'is-transient': true
            });
        }
        else {
            notification = this._notifSource.notifications[0];
            notification.body = message;
            notification.clearActions();
        }

        if (typeof transformFn === 'function') {
            transformFn(notification);
        }

        this._notifSource.addNotification(notification);
    }

    _createHistoryLabel () {
        this._historyLabel = new St.Label({
            style_class: 'ci-notification-label',
            text: ''
        });

        global.stage.add_child(this._historyLabel);

        this._historyLabel.hide();
    }

    _removeHistoryLabel () {
        if (this._historyLabel) {
            if (this._historyLabel.get_parent()) {
                global.stage.remove_child(this._historyLabel);
            }
            this._historyLabel.destroy();
            this._historyLabel = null;
        }
    }

    togglePrivateMode () {
        this.privateModeMenuItem.toggle();
    }

    _onPrivateModeSwitch () {
        PRIVATEMODE = this.privateModeMenuItem.state;
        // We hide the history in private ModeTypee because it will be out of sync (selected item will not reflect clipboard)
        this.scrollViewMenuSection.actor.visible = !PRIVATEMODE;
        this.scrollViewFavoritesMenuSection.actor.visible = !PRIVATEMODE;
        // If we get out of private mode then we restore the clipboard to old state
        if (!PRIVATEMODE) {
            let selectList = this.clipItemsRadioGroup.filter((item) => !!item.currentlySelected);

            if (selectList.length) {
                this._selectMenuItem(selectList[0]);
            } else {
                // Nothing to return to, let's empty it instead
                this.#clearClipboard();
            }

            this.#getClipboardContent().then(entry => {
                if (!entry) return;
                this.#updateIndicatorContent(entry);
            }).catch(e => console.error(e));

            this.hbox.remove_style_class_name('private-mode');
            this.#showElements();
        } else {
            this.hbox.add_style_class_name('private-mode');
            this.#updateIndicatorContent(null);
            this.#showElements();
        }
    }

    _loadSettings () {
        this._settingsChangedId = this.extension.settings.connect('changed',
            this._onSettingsChange.bind(this));

        this._fetchSettings();

        if (ENABLE_KEYBINDING)
            this._bindShortcuts();
    }

    _fetchSettings () {
        const { settings } = this.extension;
        MAX_REGISTRY_LENGTH         = settings.get_int(PrefsFields.HISTORY_SIZE);
        MAX_ENTRY_LENGTH            = settings.get_int(PrefsFields.PREVIEW_SIZE);
        DELETE_ENABLED              = settings.get_boolean(PrefsFields.DELETE);
        MOVE_ITEM_FIRST             = settings.get_boolean(PrefsFields.MOVE_ITEM_FIRST);
        NOTIFY_ON_COPY              = settings.get_boolean(PrefsFields.NOTIFY_ON_COPY);
        NOTIFY_ON_CYCLE             = settings.get_boolean(PrefsFields.NOTIFY_ON_CYCLE);
        NOTIFY_ON_CLEAR             = settings.get_boolean(PrefsFields.NOTIFY_ON_CLEAR);
        CONFIRM_ON_CLEAR            = settings.get_boolean(PrefsFields.CONFIRM_ON_CLEAR);
        CONFIRM_ON_PINNED_DELETE    = settings.get_boolean(PrefsFields.CONFIRM_ON_PINNED_DELETE);
        ENABLE_KEYBINDING           = settings.get_boolean(PrefsFields.ENABLE_KEYBINDING);
        MAX_TOPBAR_LENGTH           = settings.get_int(PrefsFields.TOPBAR_PREVIEW_SIZE);
        TOPBAR_DISPLAY_MODE         = settings.get_int(PrefsFields.TOPBAR_DISPLAY_MODE_ID);
        CLEAR_ON_BOOT               = settings.get_boolean(PrefsFields.CLEAR_ON_BOOT);
        PASTE_ON_SELECT             = settings.get_boolean(PrefsFields.PASTE_ON_SELECT);
        DISABLE_DOWN_ARROW          = settings.get_boolean(PrefsFields.DISABLE_DOWN_ARROW);
        BLINK_ICON_ON_COPY          = settings.get_boolean(PrefsFields.BLINK_ICON_ON_COPY);
        STRIP_TEXT                  = settings.get_boolean(PrefsFields.STRIP_TEXT);
        KEEP_SELECTED_ON_CLEAR      = settings.get_boolean(PrefsFields.KEEP_SELECTED_ON_CLEAR);
        PASTE_BUTTON                = settings.get_boolean(PrefsFields.PASTE_BUTTON);
        PINNED_ON_BOTTOM            = settings.get_boolean(PrefsFields.PINNED_ON_BOTTOM);
        CACHE_IMAGES                = settings.get_boolean(PrefsFields.CACHE_IMAGES);
        EXCLUDED_APPS               = settings.get_strv(PrefsFields.EXCLUDED_APPS);
        CLEAR_HISTORY_ON_INTERVAL   = settings.get_boolean(PrefsFields.CLEAR_HISTORY_ON_INTERVAL);
        CLEAR_HISTORY_INTERVAL      = settings.get_int(PrefsFields.CLEAR_HISTORY_INTERVAL);
        NEXT_HISTORY_CLEAR          = settings.get_int(PrefsFields.NEXT_HISTORY_CLEAR);
        CASE_SENSITIVE_SEARCH       = settings.get_boolean(PrefsFields.CASE_SENSITIVE_SEARCH);
        REGEX_SEARCH                = settings.get_boolean(PrefsFields.REGEX_SEARCH);
        OPEN_AT_CURSOR              = settings.get_boolean(PrefsFields.OPEN_AT_CURSOR);
        SHOW_SEARCH_BAR             = settings.get_boolean(PrefsFields.SHOW_SEARCH_BAR);
        SHOW_PRIVATE_MODE           = settings.get_boolean(PrefsFields.SHOW_PRIVATE_MODE);
        SHOW_SETTINGS_BUTTON        = settings.get_boolean(PrefsFields.SHOW_SETTINGS_BUTTON);
        SHOW_CLEAR_HISTORY_BUTTON   = settings.get_boolean(PrefsFields.SHOW_CLEAR_HISTORY_BUTTON);
        SHOW_DELETE_BUTTON          = settings.get_boolean(PrefsFields.SHOW_DELETE_BUTTON);
        SHOW_TAG_BUTTON             = settings.get_boolean(PrefsFields.SHOW_TAG_BUTTON);
        SHOW_PIN_BUTTON             = settings.get_boolean(PrefsFields.SHOW_PIN_BUTTON);
        SHOW_EDIT_BUTTON            = settings.get_boolean(PrefsFields.SHOW_EDIT_BUTTON);
        SHOW_PREVIEW_BUTTON         = settings.get_boolean(PrefsFields.SHOW_PREVIEW_BUTTON);
    }

    async _onSettingsChange () {
        try {
            // Load the settings into variables
            this._fetchSettings();

            // If the toggle is hidden but private mode is on, force it off now
            if (!SHOW_PRIVATE_MODE && PRIVATEMODE && this.privateModeMenuItem) {
                this.privateModeMenuItem.setToggleState(false);
                this._onPrivateModeSwitch();
            }

            // Remove old entries in case the registry size changed
            this._removeOldestEntries();

            // Re-set menu-items lables in case preview size changed
            this._getAllIMenuItems().forEach(mItem => {
                this._setEntryLabel(mItem);
                mItem.pasteBtn.visible = PASTE_BUTTON;
                mItem.icoBtn.visible = SHOW_DELETE_BUTTON;
                mItem.tagBtn.visible = SHOW_TAG_BUTTON;
                mItem.icofavBtn.visible = SHOW_PIN_BUTTON;
                if (mItem.editBtn) mItem.editBtn.visible = SHOW_EDIT_BUTTON;
                if (mItem.imagePreviewBtn) mItem.imagePreviewBtn.visible = SHOW_PREVIEW_BUTTON;
            });

            //update topbar
            this._updateTopbarLayout();
            this.#updateIndicatorContent(await this.#getClipboardContent());

            // Bind or unbind shortcuts
            if (ENABLE_KEYBINDING)
                this._bindShortcuts();
            else
                this._unbindShortcuts();

            // Respect UI toggles
            this.#showElements();
        } catch (e) {
            console.error('Amethystora Clipboard: Failed to update registry');
            console.error(e);
        }
    }

    _bindShortcuts () {
        this._unbindShortcuts();
        this._bindShortcut(PrefsFields.BINDING_CLEAR_HISTORY, this._removeAll);
        this._bindShortcut(PrefsFields.BINDING_PREV_ENTRY, this._previousEntry);
        this._bindShortcut(PrefsFields.BINDING_NEXT_ENTRY, this._nextEntry);
        this._bindShortcut(PrefsFields.BINDING_TOGGLE_MENU, this._toggleMenu);
        this._bindShortcut(PrefsFields.BINDING_PRIVATE_MODE, this.togglePrivateMode);
    }

    _unbindShortcuts () {
        this._shortcutsBindingIds.forEach(
            (id) => Main.wm.removeKeybinding(id)
        );

        this._shortcutsBindingIds = [];
    }

    _bindShortcut (name, cb) {
        Main.wm.addKeybinding(
            name,
            this.extension.settings,
            Meta.KeyBindingFlags.NONE,
            Shell.ActionMode.ALL,
            cb.bind(this)
        );

        this._shortcutsBindingIds.push(name);
    }

    _updateTopbarLayout () {
        if(TOPBAR_DISPLAY_MODE === 0){
            this.icon.visible = true;
            this._buttonText.visible = false;
            this._buttonImgPreview.visible = false;
            this.show();
        }
        if(TOPBAR_DISPLAY_MODE === 1){
            this.icon.visible = false;
            this._buttonText.visible = true;
            this._buttonImgPreview.visible = true;
            this.show();
        }
        if(TOPBAR_DISPLAY_MODE === 2){
            this.icon.visible = true;
            this._buttonText.visible = true;
            this._buttonImgPreview.visible = true;
            this.show();
        }
        if (TOPBAR_DISPLAY_MODE === 3) {
            this.hide();
        }
        if(!DISABLE_DOWN_ARROW) {
            this._downArrow.visible = true;
        } else {
            this._downArrow.visible = false;
        }
    }

    _disconnectSettings () {
        if (!this._settingsChangedId)
            return;

        this.extension.settings.disconnect(this._settingsChangedId);
        this._settingsChangedId = null;

        if (this._intervalSettingChangedId) {
            this.extension.settings.disconnect(this._intervalSettingChangedId);
            this._intervalSettingChangedId = null;
        }

        if (this._intervalToggleChangedId) {
            this.extension.settings.disconnect(this._intervalToggleChangedId);
            this._intervalToggleChangedId = null;
        }

        if (this._historyClearTimeoutId) {
            clearTimeout(this._historyClearTimeoutId);
            this._historyClearTimeoutId = null;
        }
    }

    _disconnectSelectionListener () {
        if (!this._selectionOwnerChangedId)
            return;

        this.selection.disconnect(this._selectionOwnerChangedId);
    }

    _clearDelayedSelectionTimeout () {
        if (this._delayedSelectionTimeoutId) {
            clearInterval(this._delayedSelectionTimeoutId);
        }
    }

    _selectEntryWithDelay (entry) {
        this._selectMenuItem(entry, false);

        this._delayedSelectionTimeoutId = setTimeout(() => {
            this._selectMenuItem(entry);  //select the item
            this._delayedSelectionTimeoutId = null;
        }, DELAYED_SELECTION_TIMEOUT);
    }

    _previousEntry () {
        if (PRIVATEMODE) return;

        this._clearDelayedSelectionTimeout();

        this._getAllIMenuItems().some((mItem, i, menuItems) => {
            if (mItem.currentlySelected) {
                i--;                                 //get the previous index
                if (i < 0) i = menuItems.length - 1; //cycle if out of bound
                let index = i + 1;                   //index to be displayed

                if(NOTIFY_ON_CYCLE) {
                    this._showNotification(index + ' / ' + menuItems.length + ': ' + menuItems[i].entry.getStringValue());
                }
                if (MOVE_ITEM_FIRST) {
                    this._selectEntryWithDelay(menuItems[i]);
                }
                else {
                    this._selectMenuItem(menuItems[i]);
                }
                return true;
            }
            return false;
        });
    }

    _nextEntry () {
        if (PRIVATEMODE) return;

        this._clearDelayedSelectionTimeout();

        this._getAllIMenuItems().some((mItem, i, menuItems) => {
            if (mItem.currentlySelected) {
                i++;                                 //get the next index
                if (i === menuItems.length) i = 0;   //cycle if out of bound
                let index = i + 1;                     //index to be displayed

                if(NOTIFY_ON_CYCLE) {
                    this._showNotification(index + ' / ' + menuItems.length + ': ' + menuItems[i].entry.getStringValue());
                }
                if (MOVE_ITEM_FIRST) {
                    this._selectEntryWithDelay(menuItems[i]);
                }
                else {
                    this._selectMenuItem(menuItems[i]);
                }
                return true;
            }
            return false;
        });
    }

    _toggleMenu () {
        if (!this.menu.isOpen && OPEN_AT_CURSOR) {
            const [x, y] = global.get_pointer();
            this._cursorActor.set_position(x, y);
            this.menu.sourceActor = this._cursorActor;
        }
        this.menu.toggle();
    }




    #pasteItem (menuItem) {
        this.menu.close();
        const currentlySelected = this._getCurrentlySelectedItem();
        this.preventIndicatorUpdate = true;
        this.#updateClipboard(menuItem.entry);
        this._pastingKeypressTimeout = setTimeout(() => {
            if (this.keyboard.purpose === Clutter.InputContentPurpose.TERMINAL) {
                this.keyboard.press(Clutter.KEY_Control_L);
                this.keyboard.press(Clutter.KEY_Shift_L);
                this.keyboard.press(Clutter.KEY_Insert);
                this.keyboard.release(Clutter.KEY_Insert);
                this.keyboard.release(Clutter.KEY_Shift_L);
                this.keyboard.release(Clutter.KEY_Control_L);
            }
            else {
                this.keyboard.press(Clutter.KEY_Shift_L);
                this.keyboard.press(Clutter.KEY_Insert);
                this.keyboard.release(Clutter.KEY_Insert);
                this.keyboard.release(Clutter.KEY_Shift_L);
            }

            this._pastingResetTimeout = setTimeout(() => {
                this.preventIndicatorUpdate = false;
                if (currentlySelected && currentlySelected.entry)
                    this.#updateClipboard(currentlySelected.entry);
            }, 50);
        }, 50);
    }

    #showImagePreview (entry, onClose = null) {
        this.#closeImagePreview();
        this.menu.close();

        const monitor = Main.layoutManager.currentMonitor;

        const overlay = new St.Widget({
            reactive: true,
            can_focus: true,
            x: monitor.x,
            y: monitor.y,
            width: monitor.width,
            height: monitor.height,
            style: 'background-color: rgba(0, 0, 0, 0.75);',
        });

        this.#_imagePreviewOverlay = overlay;
        global.stage.add_child(overlay);
        overlay.grab_key_focus();

        const close = () => {
            this.#closeImagePreview();
            if (onClose) onClose();
        };

        overlay._previewClickId = overlay.connect('button-press-event', () => {
            close();
            return Clutter.EVENT_STOP;
        });

        overlay._previewKeyId = overlay.connect('key-press-event', (_actor, event) => {
            if (event.get_key_symbol() === Clutter.KEY_Escape) {
                close();
                return Clutter.EVENT_STOP;
            }
            return Clutter.EVENT_PROPAGATE;
        });

        const maxW = Math.floor(monitor.width * 0.5);
        const maxH = Math.floor(monitor.height * 0.4);

        const bin = new St.Bin({
            x_align: Clutter.ActorAlign.CENTER,
            y_align: Clutter.ActorAlign.CENTER,
        });
        bin.add_constraint(new Clutter.AlignConstraint({
            source: overlay,
            align_axis: Clutter.AlignAxis.X_AXIS,
            factor: 0.5,
        }));
        bin.add_constraint(new Clutter.AlignConstraint({
            source: overlay,
            align_axis: Clutter.AlignAxis.Y_AXIS,
            factor: 0.5,
        }));
        overlay.add_child(bin);

        // From memory, at the picture's own proportions: there is no file of it in the clear to load at its own size
        const actor = this.registry.getEntryAsPicture(entry, maxW, maxH);
        if (actor)
            bin.set_child(actor);
        else
            console.error('Amethystora Clipboard: failed to load image preview');
    }

    #showTagDialog (menuItem, reopenOnClose = false) {
        const dialog = new ModalDialog.ModalDialog({ destroyOnClose: true });

        const onDialogClose = () => {
            if (reopenOnClose) {
                this._focusItemOnOpen = menuItem;
                this.menu.open();
            }
        };

        const textEntry = new St.Entry({
            text: menuItem.entry.getTag() || '',
            hint_text: _('Enter tag…'),
            can_focus: true,
            x_expand: true,
            style: 'min-width: 300px;',
        });

        dialog.contentLayout.add_child(textEntry);

        dialog.addButton({
            label: _('Discard'),
            action: () => {
                dialog.close();
                onDialogClose();
            },
            key: Clutter.KEY_Escape,
        });

        dialog.addButton({
            label: _('Save'),
            action: () => {
                const tag = textEntry.get_text().trim() || null;
                menuItem.entry.setTag(tag);
                this._updateTagLabel(menuItem);
                this._updateCache();
                dialog.close();
                onDialogClose();
            },
            default: true,
        });

        dialog.open();
        textEntry.grab_key_focus();
    }

    _updateTagLabel (menuItem) {
        if (menuItem.tagLabel) {
            menuItem.actor.remove_child(menuItem.tagLabel);
            menuItem.tagLabel.destroy();
            menuItem.tagLabel = null;
        }

        const tag = menuItem.entry.getTag();
        if (tag) {
            menuItem.tagLabel = new St.Label({
                text: tag,
                style_class: 'ci-tag-label',
                y_align: Clutter.ActorAlign.CENTER,
            });
            menuItem.actor.insert_child_above(menuItem.tagLabel, menuItem.label);
        }
    }

    #showEditDialog (menuItem, reopenOnClose = false) {
        const dialog = new ModalDialog.ModalDialog({ destroyOnClose: true });

        const onDialogClose = () => {
            if (reopenOnClose) {
                this._focusItemOnOpen = menuItem;
                this.menu.open();
            }
        };

        const scrollView = new St.ScrollView({
            hscrollbar_policy: St.PolicyType.NEVER,
            vscrollbar_policy: St.PolicyType.AUTOMATIC,
            x_expand: true,
            y_expand: false,
            style: 'min-width: 400px; min-height: 100px; max-height: 400px;',
        });

        const clutterText = new Clutter.Text({
            text: menuItem.entry.getStringValue(),
            editable: true,
            reactive: true,
            single_line_mode: false,
            activatable: false,
            line_wrap: true,

        });

        const white = new Cogl.Color();
        white.init_from_4f(1.0, 1.0, 1.0, 1.0);
        const selectionBlue = new Cogl.Color();
        selectionBlue.init_from_4f(0.39, 0.59, 1.0, 0.71);
        clutterText.color = white;
        clutterText.selection_color = selectionBlue;
        clutterText.selected_text_color = white;

        const textBox = new St.BoxLayout({
            style_class: 'ci-edit-textbox',
            x_expand: true,
            y_expand: true,
            orientation: Clutter.Orientation.VERTICAL,
        });

        textBox.add_child(clutterText);

        scrollView.add_child(textBox);
        dialog.contentLayout.add_child(scrollView);

        dialog.addButton({
            label: _('Discard'),
            action: () => {
                dialog.close();
                onDialogClose();
            },
            key: Clutter.KEY_Escape,
        });

        dialog.addButton({
            label: _('Save'),
            action: () => {
                const newText = clutterText.get_text();
                menuItem.entry.setText(newText);
                menuItem.clipContents = newText;
                this._setEntryLabel(menuItem);
                this._updateCache();
                if (menuItem.currentlySelected)
                    this.#updateClipboard(menuItem.entry);
                dialog.close();
                onDialogClose();
            },
            default: true,
        });

        if (reopenOnClose) this.menu.close();
        dialog.open();
        clutterText.grab_key_focus();
    }

    // Make a new copy: the item's text actions, each result a new item at the top and on the clipboard
    #showTextActions (menuItem, reopenOnClose = false) {
        const text = menuItem.entry.getStringValue();
        this.menu.close();
        this._textActionsDialog?.close();
        const dialog = new TextActionsDialog(text, result => this.#copyAsNew(result), () => {
            if (this._textActionsDialog === dialog) this._textActionsDialog = null;
            if (reopenOnClose && !this._destroyed) {
                this._focusItemOnOpen = menuItem;
                this.menu.open();
            }
        });
        this._textActionsDialog = dialog;
        dialog.open();
    }

    #copyAsNew (text) {
        if (this._destroyed) return;
        const entry = new ClipboardEntry('text/plain;charset=utf-8', new TextEncoder().encode(text), false);
        const known = this.clipItemsRadioGroup.find(item => item.entry.equals(entry));
        if (known) {
            this._selectMenuItem(known, true);
            return;
        }
        if (PRIVATEMODE) {
            this.extension.clipboard.set_text(CLIPBOARD_TYPE, text);
            return;
        }
        this.#addToCache(entry);
        aboutEntry(entry).copiedAt = GLib.DateTime.new_now_local();
        this._addEntry(entry, true, true);
        this._removeOldestEntries();
    }

    #closeImagePreview () {
        if (!this.#_imagePreviewOverlay) return;

        const overlay = this.#_imagePreviewOverlay;
        this.#_imagePreviewOverlay = null;

        if (overlay._previewClickId) overlay.disconnect(overlay._previewClickId);
        if (overlay._previewKeyId) overlay.disconnect(overlay._previewKeyId);

        if (overlay.get_parent()) global.stage.remove_child(overlay);
        overlay.destroy();
    }

    #clearTimeouts () {
        if (this._imagePreviewTimeout) clearTimeout(this._imagePreviewTimeout);
        if (this._setFocusOnOpenTimeout) clearTimeout(this._setFocusOnOpenTimeout);
        if (this._pastingKeypressTimeout) clearTimeout(this._pastingKeypressTimeout);
        if (this._pastingResetTimeout) clearTimeout(this._pastingResetTimeout);
        if (this._historyClearTimeoutId) clearTimeout(this._historyClearTimeoutId);
        if (this._timerIntervalId) clearInterval(this._timerIntervalId);
        if (this._blinkAnimationTimeout) clearTimeout(this._blinkAnimationTimeout);
    }

    #clearClipboard () {
        this.extension.clipboard.set_text(CLIPBOARD_TYPE, "");
        this.#updateIndicatorContent(null);
    }

    #updateClipboard (entry) {
        this.extension.clipboard.set_content(CLIPBOARD_TYPE, entry.mimetype(), entry.asBytes());
        this.#updateIndicatorContent(entry);
    }

    // What the clipboard holds as TYPE, its bytes copied (an empty array for nothing); given up after a while, since a
    // copy is taken one at a time and an app that never answers would keep every later copy from being recorded.
    // The GLib.Bytes St hands the callback is freed once it returns: only its data, copied in it, may outlive it
    #read (type) {
        return new Promise(resolve => {
            let answered = false;
            const timer = setTimeout(() => {
                answered = true;
                console.warn(`Amethystora Clipboard: the app that copied did not send ${type} in ${CLIPBOARD_READ_TIMEOUT / 1000} s`);
                resolve(new Uint8Array());
            }, CLIPBOARD_READ_TIMEOUT);
            this.extension.clipboard.get_content(CLIPBOARD_TYPE, type, (clipBoard, bytes) => {
                if (answered) return;
                answered = true;
                clearTimeout(timer);
                resolve(bytes?.get_data() ?? new Uint8Array());
            });
        });
    }

    async #getClipboardContent () {
        const offered = this.extension.clipboard.get_mimetypes(CLIPBOARD_TYPE);
        // A password a password manager marks secret is neither recorded nor shown in the top bar; so is one whose
        // mark comes empty or never comes, as from a GTK app, which offers the type but cannot send it
        if (offered.includes(PASSWORD_HINT)) {
            const hint = new TextDecoder().decode(await this.#read(PASSWORD_HINT)).trim();
            if (hint === 'secret' || hint === '') return null;
        }

        const mimetypes = [
            "text/plain;charset=utf-8",
            "UTF8_STRING",
            "text/plain",
            "STRING",
            'image/gif',
            'image/png',
            'image/jpg',
            'image/jpeg',
            'image/webp',
            'image/svg+xml',
            'text/html',
        ];

        for (let type of mimetypes) {
            // Only what the app offers: asked for a type it did not offer, a Wayland app may send what it has
            // anyway (a picture's bytes taken for text), or never answer (and nothing is recorded again)
            if (!offered.includes(type)) continue;
            const data = await this.#read(type);
            let result = null;
            // A one-time code Amethystora UI's pill copied (its modules/pill/code.js) is never recorded
            const secret = globalThis[Symbol.for('amethystora.clipboard.secret')];
            if (secret && new TextDecoder().decode(data) === secret) return null;
            // Too large to keep or to decode in the shell (the header is read, never the picture): the copy is skipped
            if (data.length > 0 && !copyFits(type.startsWith('image/'), data)) {
                console.warn(`Amethystora Clipboard: a copy of ${type} was too large to record`);
                return null;
            }
            if (data.length > 0) {
                // HACK: workaround for GNOME 2nd+ copy mangling mimetypes https://gitlab.gnome.org/GNOME/gnome-shell/-/issues/8233
                // In theory GNOME or XWayland should auto-convert this back to UTF8_STRING for legacy apps when it's needed https://gitlab.gnome.org/GNOME/gtk/-/merge_requests/5300
                if (type === "UTF8_STRING") {
                    type = "text/plain;charset=utf-8";
                }

                // A picture is sealed with the history (registry.js), never written as it is
                result = new ClipboardEntry(type, data, false);
            }

            if (result) {
                if (!CACHE_IMAGES && result.isImage()) {
                    return null;
                }
                else {
                    return result;
                }
            }
        }

        return null;
    }
});
