//! Send to nearby (R7-27): a file that another computer sends waits for the person's Accept. Nothing arrives by itself.
//!
//! The protocol has no ask: a sender offers a file by telling where to fetch it. So the offer is held, and the card
//! says who sends what; Accept fetches it as any other file, and a no (Decline, closing the card, or a minute with no
//! answer) tells the sender to stand down, as every refusal here does. The key `ask-first` says whose files wait:
//! `computers` (the default: another computer is not a phone the person holds), `all`, or `never`, which is how files
//! arrived before.
//!
//! An Accept covers the rest of the batch the sender announced (`numberOfFiles`), ten minutes at most, so that a
//! folder of files is one question. A held offer is a packet and a connection and nothing else: no file name is
//! written anywhere, and the card is the program's own notification as every other.

use std::rc::Rc;
use std::time::{Duration, Instant};

use crate::device::Device;
use crate::identity;
use crate::link::Link;
use crate::notify::Notice;
use crate::packet::Packet;
use crate::service::Service;
use crate::share::Batch;
use crate::text::{self, tr, trf};

/// How long a card waits for an answer
pub const WAIT: Duration = Duration::from_secs(60);
/// How long one Accept covers the rest of its batch
const COVER: Duration = Duration::from_secs(600);
/// Offers held for one device; the rest are refused at once
const MOST_HELD: usize = 100;

/// Whose files wait for Accept (the GSettings key ask-first)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AskFirst {
    /// Other computers' files; a phone the person pairs and holds is trusted to send
    #[default]
    Computers,
    All,
    /// Files arrive as they did before this existed
    Never,
}

impl AskFirst {
    /// The key's value; anything unknown is the default
    pub fn from_key(value: &str) -> AskFirst {
        match value {
            "all" => AskFirst::All,
            "never" => AskFirst::Never,
            _ => AskFirst::Computers,
        }
    }

    pub fn applies(self, kind: &str) -> bool {
        match self {
            AskFirst::Computers => identity::is_computer(kind),
            AskFirst::All => true,
            AskFirst::Never => false,
        }
    }
}

struct Held {
    packet: Packet,
    link: Rc<Link>,
    name: String,
    modified: Option<i64>,
    batch: Option<Batch>,
}

struct Allow {
    remaining: u32,
    until: Instant,
}

/// A device's offers that wait, and what its last Accept still covers
#[derive(Default)]
pub struct Asking {
    held: Vec<Held>,
    /// Which card the held offers belong to; a later one is a new question
    round: u64,
    /// How many offers the card on the screen said when it was last drawn: an Accept covers those, and not what came after
    shown: usize,
    allow: Option<Allow>,
}

impl Asking {
    /// Uses up one file of what an Accept covered, if any is left
    fn covered(&mut self) -> bool {
        let Some(allow) = self.allow.as_mut() else { return false };
        if Instant::now() >= allow.until || allow.remaining == 0 {
            self.allow = None;
            return false;
        }
        allow.remaining -= 1;
        if allow.remaining == 0 {
            self.allow = None;
        }
        true
    }

    #[cfg(test)]
    fn cover(&mut self, remaining: u32) {
        self.allow = Some(Allow { remaining, until: Instant::now() + COVER });
    }
}

impl Service {
    /// True when the offer is held for the person's Accept, so that the caller does nothing more with it
    pub(crate) fn hold_for_accept(self: &Rc<Self>, dev: &Rc<Device>, link: &Rc<Link>, p: &Packet, name: &str, modified: Option<i64>, batch: Option<Batch>) -> bool {
        let kind = dev.st.borrow().kind.clone();
        if !self.ask_first.get().applies(&kind) {
            return false;
        }
        let round;
        let first;
        {
            let mut st = dev.st.borrow_mut();
            if st.asking.covered() {
                return false;
            }
            if st.asking.held.len() >= MOST_HELD {
                drop(st);
                self.refuse_payload(link, dev, p.payload().ok().flatten());
                return true;
            }
            first = st.asking.held.is_empty();
            if first {
                st.asking.round = self.next_id();
            }
            round = st.asking.round;
            st.asking.held.push(Held { packet: p.clone(), link: link.clone(), name: name.to_string(), modified, batch });
        }
        {
            let mut st = dev.st.borrow_mut();
            st.asking.shown = st.asking.held.len();
        }
        self.raise_ask(dev, round);
        if first {
            self.start_ask_timer(dev, round);
        }
        true
    }

    /// A card is answered, by the person or by the minute, only once
    fn start_ask_timer(self: &Rc<Self>, dev: &Rc<Device>, round: u64) {
        let service = self.clone();
        let id = dev.id.clone();
        self.spawn_task(async move {
            glib::timeout_future(WAIT).await;
            if let Some(dev) = service.device(&id) {
                // Only the question this timer was started for; a later one has a timer of its own
                if dev.st.borrow().asking.round == round {
                    service.answer_ask(&dev, round, false);
                }
            }
        });
    }

    fn raise_ask(&self, dev: &Rc<Device>, round: u64) {
        let (files, bytes, single) = {
            let st = dev.st.borrow();
            let held = &st.asking.held;
            let announced = held.first().and_then(|h| h.batch);
            let bytes: u64 = held.iter().map(|h| h.packet.declared_size().unwrap_or(0)).sum();
            (
                announced.map_or(held.len() as u32, |b| b.files.max(held.len() as u32)),
                announced.map_or(bytes, |b| if b.bytes > 0 { b.bytes } else { bytes }),
                held.first().map(|h| h.name.clone()),
            )
        };
        let name = dev.name();
        let (summary, body) = if files <= 1 {
            (
                trf("“{name}” wants to send you a file", &[("name", &name)]),
                format!("{} · {}", text::file_name(&single.unwrap_or_default()), glib::format_size(bytes)),
            )
        } else {
            (trf("“{name}” wants to send you files", &[("name", &name)]), trf("{count} files · {size}", &[("count", &files.to_string()), ("size", &glib::format_size(bytes))]))
        };
        self.raise(Notice {
            key: format!("ask:{}:{round}", dev.id),
            summary,
            body,
            icon: Some("document-send-symbolic".into()),
            actions: vec![("accept".to_string(), tr("Accept")), ("decline".to_string(), tr("Decline"))],
            // The card goes by itself when the minute is up, which is the answer's own timer
            timeout_ms: WAIT.as_millis() as i32,
            ..Notice::default()
        });
    }

    /// The person pressed a button of the card, or closed it
    pub fn on_ask_event(self: &Rc<Self>, dev: &Rc<Device>, rest: &str, what: &str) {
        let Ok(round) = rest.parse::<u64>() else { return };
        self.answer_ask(dev, round, what == "accept");
    }

    /// Accept fetches what is held; anything else sends the senders away. Only for the question it was asked of.
    fn answer_ask(self: &Rc<Self>, dev: &Rc<Device>, round: u64, accept: bool) {
        let (held, left) = {
            let mut st = dev.st.borrow_mut();
            if st.asking.round != round || st.asking.held.is_empty() {
                return;
            }
            // Accept covers what the card said; offers that came while it was up wait for a card of their own. No answers all.
            let cut = if accept { st.asking.shown.clamp(1, st.asking.held.len()) } else { st.asking.held.len() };
            let later = st.asking.held.split_off(cut);
            let held = std::mem::replace(&mut st.asking.held, later);
            let left = st.asking.held.len();
            if accept {
                // The rest of the batch the sender announced is covered, and nothing else
                let announced = held.first().and_then(|h| h.batch).map_or(0, |b| b.files);
                let rest = announced.saturating_sub((held.len() + left) as u32);
                st.asking.allow = (rest > 0).then(|| Allow { remaining: rest, until: Instant::now() + COVER });
            }
            (held, left)
        };
        self.notifier.close(&format!("ask:{}:{round}", dev.id));
        if left > 0 {
            let next = self.next_id();
            {
                let mut st = dev.st.borrow_mut();
                st.asking.round = next;
                st.asking.shown = left;
            }
            self.raise_ask(dev, next);
            self.start_ask_timer(dev, next);
        }
        for h in held {
            if accept {
                self.receive_file_now(dev, &h.link, &h.packet, h.name, h.modified, h.batch);
            } else {
                self.refuse_payload(&h.link, dev, h.packet.payload().ok().flatten());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn who_is_asked() {
        assert!(AskFirst::Computers.applies("laptop") && AskFirst::Computers.applies("desktop"));
        assert!(!AskFirst::Computers.applies("phone") && !AskFirst::Computers.applies("tablet"));
        assert!(AskFirst::All.applies("phone"));
        assert!(!AskFirst::Never.applies("laptop"));
        assert_eq!(AskFirst::from_key("nonsense"), AskFirst::Computers);
        assert_eq!(AskFirst::from_key("never"), AskFirst::Never);
    }

    #[test]
    fn an_accept_covers_the_rest_of_a_batch_and_no_more() {
        let mut asking = Asking::default();
        assert!(!asking.covered(), "nothing is covered before anyone accepted");
        asking.cover(2);
        assert!(asking.covered());
        assert!(asking.covered());
        assert!(!asking.covered(), "the third file is a new question");
    }
}
