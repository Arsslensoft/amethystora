//! Texts: the phone's conversations and messages, kept in memory only (spec-plugins.md 8). The phone answers a
//! request for its conversations with the newest message of each thread (the digest), a request for one
//! thread with that thread's messages, and tells of a new message by itself; all three are the same packet and
//! are merged here by the message's id. Nothing is written anywhere.

use std::collections::BTreeMap;

use std::collections::HashSet;

use serde_json::{Map, Value, json};

use crate::limits;
use crate::packet;
use crate::text;

/// The placeholder some phones put where an address was not known
const PLACEHOLDER: &str = "insert-address-token";

// What one phone may make this program hold and do. A digest has a message for each of up to 1,000 threads and
// a thread's reply up to 500, so 5,000 in one packet is more than an honest phone sends; the rest is dropped.
const MESSAGES_PER_PACKET: usize = 5_000;
/// Threads whose change is told one by one on the bus; past this the conversation list is told to look again
pub const THREAD_SIGNALS: usize = 64;
const ADDRESSES_PER_MESSAGE: usize = 16;
const ADDRESS_LENGTH: usize = 64;
/// How old a text may be, by its date, to be fresh: two minutes either way, since the phone's clock is its own
pub const FRESH_MS: i64 = 2 * 60 * 1000;
/// All the texts of a phone together: the oldest conversations go first when they pass it
const BYTES_KEPT: usize = 32 * 1024 * 1024;

/// What a message holds in memory, near enough
fn cost(m: &Message) -> usize {
    m.body.len() + m.addresses.iter().map(|a| a.len() + 24).sum::<usize>() + 96
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub id: i64,
    pub thread: i64,
    /// The sender first when incoming; every recipient when outgoing
    pub addresses: Vec<String>,
    pub body: String,
    pub date: i64,
    /// Android's box: 1 inbox, 2 sent, 4 outbox, 5 failed, 6 queued
    pub kind: i32,
    pub read: bool,
    pub attachments: bool,
}

impl Message {
    pub fn outgoing(&self) -> bool {
        matches!(self.kind, 2 | 4 | 5 | 6)
    }

    pub fn status(&self) -> &'static str {
        match self.kind {
            4 | 6 => "sending",
            5 => "failed",
            2 => "sent",
            _ => "received",
        }
    }
}

#[derive(Debug)]
pub struct Sms {
    threads: BTreeMap<i64, Vec<Message>>,
    /// The sum of `cost` over every message kept
    bytes: usize,
    /// What may be kept in all: `BYTES_KEPT`, less in a test
    budget: usize,
}

impl Default for Sms {
    fn default() -> Self {
        Sms { threads: BTreeMap::new(), bytes: 0, budget: BYTES_KEPT }
    }
}

/// What a packet changed
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Changes {
    pub threads: Vec<i64>,
    /// Whether the list of conversations changed (a thread came, went, or got a newer message)
    pub conversations: bool,
    /// The bodies of the texts that came in just now and were not here before, unread: filled only when the caller
    /// asked (a code from your phone), and kept for no other purpose
    pub fresh: Vec<String>,
}

fn message_of(value: &Value) -> Option<Message> {
    let object = value.as_object()?;
    let kind = packet::int_of(object, "type")? as i32;
    // Drafts and the all-boxes number are not conversations
    if !matches!(kind, 1 | 2 | 4 | 5 | 6) {
        return None;
    }
    let addresses: Vec<String> = object
        .get("addresses")?
        .as_array()?
        .iter()
        .filter_map(|a| a.get("address").and_then(Value::as_str))
        .filter(|a| !a.is_empty() && *a != PLACEHOLDER)
        .take(ADDRESSES_PER_MESSAGE)
        .map(|a| text::shorten(&text::clean_line(a), ADDRESS_LENGTH))
        .collect();
    if addresses.is_empty() {
        return None;
    }
    let date = packet::int_of(object, "date")?;
    Some(Message {
        id: packet::int_of(object, "_id").unwrap_or(date),
        thread: packet::int_of(object, "thread_id")?,
        addresses,
        body: text::shorten(&text::clean(packet::str_of(object, "body").unwrap_or("")), limits::MESSAGE_BODY),
        date,
        kind,
        read: packet::int_of(object, "read") == Some(1),
        attachments: object.get("attachments").and_then(Value::as_array).is_some_and(|a| !a.is_empty()),
    })
}

impl Sms {
    pub fn is_empty(&self) -> bool {
        self.threads.is_empty()
    }

    fn drop_thread(&mut self, thread: i64) {
        if let Some(messages) = self.threads.remove(&thread) {
            self.bytes = self.bytes.saturating_sub(messages.iter().map(cost).sum());
        }
    }

    /// A `kdeconnect.sms.messages` body
    pub fn apply(&mut self, body: &Map<String, Value>) -> Changes {
        self.apply_looking(body, None)
    }

    /// The same, and with `near` (the time in milliseconds) the bodies of the incoming, unread texts that were not here
    /// before and are dated within `FRESH_MS` of it come back in `Changes::fresh`. A digest, a reply to a request for older
    /// messages and a text already known are none of them fresh, by the date and the id.
    pub fn apply_looking(&mut self, body: &Map<String, Value>, near: Option<i64>) -> Changes {
        let mut changes = Changes::default();
        let Some(list) = body.get("messages").and_then(Value::as_array).filter(|l| !l.is_empty()) else { return changes };
        let incoming: Vec<Message> = list.iter().take(MESSAGES_PER_PACKET).filter_map(message_of).collect();
        let seen: HashSet<i64> = incoming.iter().map(|m| m.thread).collect();
        let mut reported: HashSet<i64> = HashSet::new();
        let mut tell = |changes: &mut Changes, thread: i64| {
            if reported.insert(thread) {
                if changes.threads.len() < THREAD_SIGNALS {
                    changes.threads.push(thread);
                } else {
                    changes.conversations = true;
                }
            }
        };
        // More than one thread in a reply is the digest: the threads it leaves out are gone from the phone
        if seen.len() > 1 {
            let gone: Vec<i64> = self.threads.keys().filter(|t| !seen.contains(t)).copied().collect();
            for thread in gone {
                self.drop_thread(thread);
                tell(&mut changes, thread);
                changes.conversations = true;
            }
        }
        for message in incoming {
            let thread = message.thread;
            let known = self.threads.contains_key(&thread);
            let newest_before = self.threads.get(&thread).and_then(|m| m.last()).map(|m| (m.date, m.id));
            let read_on_phone = message.read;
            let messages = self.threads.entry(thread).or_default();
            let mut flipped = false;
            if read_on_phone {
                // The phone has read the thread up to here
                for old in messages.iter_mut().filter(|m| m.date <= message.date && !m.read) {
                    old.read = true;
                    flipped = true;
                }
            }
            let added = cost(&message);
            let mut removed = 0;
            match messages.iter_mut().find(|m| m.id == message.id) {
                Some(existing) => {
                    removed += cost(existing);
                    *existing = message;
                }
                None => {
                    if near.is_some_and(|now| message.kind == 1 && !message.read && (now - message.date).abs() <= FRESH_MS) {
                        changes.fresh.push(message.body.clone());
                    }
                    messages.push(message);
                }
            }
            messages.sort_by_key(|m| (m.date, m.id));
            if messages.len() > limits::MESSAGES_PER_THREAD {
                let extra = messages.len() - limits::MESSAGES_PER_THREAD;
                removed += messages.drain(..extra).map(|m| cost(&m)).sum::<usize>();
            }
            let newest_after = messages.last().map(|m| (m.date, m.id));
            self.bytes = (self.bytes + added).saturating_sub(removed);
            tell(&mut changes, thread);
            if !known || newest_after != newest_before || flipped {
                changes.conversations = true;
            }
        }
        if self.threads.len() > limits::THREADS_KEPT || self.bytes > self.budget {
            let mut by_age: Vec<(i64, i64)> = self.threads.iter().map(|(t, m)| (m.last().map_or(0, |m| m.date), *t)).collect();
            by_age.sort();
            for (_, thread) in by_age {
                if self.threads.len() <= limits::THREADS_KEPT && self.bytes <= self.budget {
                    break;
                }
                self.drop_thread(thread);
                changes.conversations = true;
            }
        }
        changes
    }

    /// The participants of a thread: those of its newest message
    pub fn participants(&self, thread: i64) -> Option<&[String]> {
        self.threads.get(&thread)?.last().map(|m| m.addresses.as_slice())
    }

    /// The conversations, newest first, at most `limit`, as the bus gives them
    pub fn conversations(&self, limit: usize, name_of: &dyn Fn(&str) -> Option<String>) -> Vec<Map<String, Value>> {
        let mut threads: Vec<(&i64, &Message)> = self.threads.iter().filter_map(|(t, m)| m.last().map(|last| (t, last))).collect();
        threads.sort_by_key(|(_, last)| std::cmp::Reverse((last.date, last.id)));
        threads
            .into_iter()
            .take(limit)
            .map(|(thread, last)| {
                let names: Vec<String> = last.addresses.iter().map(|a| name_of(a).unwrap_or_else(|| a.clone())).collect();
                let name = match names.as_slice() {
                    [one] => one.clone(),
                    [first, rest @ ..] => text::trf("{name} and {count} more", &[("name", first), ("count", &rest.len().to_string())]),
                    [] => String::new(),
                };
                let snippet = if last.body.is_empty() && last.attachments { text::tr("Attachment") } else { text::shorten(&text::clean_line(&last.body), 120) };
                let mut c = Map::new();
                c.insert("thread_id".into(), json!(thread));
                c.insert("addresses".into(), json!(last.addresses));
                c.insert("name".into(), json!(name));
                c.insert("snippet".into(), json!(snippet));
                c.insert("date".into(), json!(last.date));
                c.insert("unread".into(), json!(!last.outgoing() && !last.read));
                c.insert("group".into(), json!(last.addresses.len() > 1));
                c
            })
            .collect()
    }

    /// A thread's messages, oldest first, the newest `limit` of them
    pub fn messages(&self, thread: i64, limit: usize, name_of: &dyn Fn(&str) -> Option<String>) -> Vec<Map<String, Value>> {
        let Some(all) = self.threads.get(&thread) else { return Vec::new() };
        let skip = all.len().saturating_sub(limit);
        all[skip..]
            .iter()
            .map(|m| {
                let address = m.addresses[0].clone();
                let body = if m.body.is_empty() && m.attachments { text::tr("Attachment") } else { m.body.clone() };
                let mut v = Map::new();
                v.insert("id".into(), json!(m.id));
                v.insert("thread_id".into(), json!(m.thread));
                v.insert("name".into(), json!(name_of(&address).unwrap_or_else(|| address.clone())));
                v.insert("address".into(), json!(address));
                v.insert("body".into(), json!(body));
                v.insert("date".into(), json!(m.date));
                v.insert("outgoing".into(), json!(m.outgoing()));
                v.insert("read".into(), json!(m.outgoing() || m.read));
                v.insert("status".into(), json!(m.status()));
                v
            })
            .collect()
    }
}

/// The `kdeconnect.sms.request_conversation` body for a thread; with `before` and `count` for older messages
pub fn thread_request(thread: i64, before: i64, count: i32) -> Value {
    if before > 0 && count > 0 {
        json!({ "threadID": thread, "rangeStartTimestamp": before, "numberToRequest": count })
    } else {
        json!({ "threadID": thread })
    }
}

/// The `kdeconnect.sms.request` body: the documented form (version 2, a list of addresses) for a phone that
/// speaks the current protocol, the older single-recipient form otherwise. One place to change it when the
/// real phone says which it takes.
pub fn send_request(addresses: &[String], body: &str, documented: bool) -> Value {
    if documented {
        json!({ "version": 2, "addresses": addresses.iter().map(|a| json!({ "address": a })).collect::<Vec<_>>(), "messageBody": body })
    } else {
        json!({ "sendSms": true, "phoneNumber": addresses.first().cloned().unwrap_or_default(), "messageBody": body })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    fn no_names(_: &str) -> Option<String> {
        None
    }

    fn digest() -> Map<String, Value> {
        body(json!({ "version": 2, "messages": [
            { "_id": 900, "thread_id": 42, "addresses": [ { "address": "+15550100" } ], "body": "See you at 8", "date": 1761253000000i64, "type": 1, "read": 0, "event": 1, "sub_id": 1 },
            { "_id": 880, "thread_id": 7, "addresses": [ { "address": "+15550102" } ], "body": "Thanks!", "date": 1761200000000i64, "type": 2, "read": 1, "event": 1, "sub_id": 1 } ] }))
    }

    #[test]
    fn the_digest_makes_the_conversation_list() {
        let mut sms = Sms::default();
        let changes = sms.apply(&digest());
        assert!(changes.conversations);
        let list = sms.conversations(10, &no_names);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0]["thread_id"], 42, "newest first");
        assert_eq!(list[0]["snippet"], "See you at 8");
        assert_eq!(list[0]["unread"], true);
        assert_eq!(list[0]["group"], false);
        assert_eq!(list[1]["unread"], false, "the last message was ours");
        assert_eq!(sms.conversations(1, &no_names).len(), 1);
    }

    #[test]
    fn a_digest_drops_the_threads_it_leaves_out_and_an_empty_reply_nothing() {
        let mut sms = Sms::default();
        sms.apply(&digest());
        sms.apply(&body(json!({ "messages": [ { "_id": 1, "thread_id": 99, "addresses": [ { "address": "555" } ], "body": "x", "date": 5, "type": 1, "read": 0 } ] })));
        assert_eq!(sms.conversations(10, &no_names).len(), 3, "one thread alone is an update, not a digest");
        sms.apply(&digest());
        assert_eq!(sms.conversations(10, &no_names).len(), 2, "99 is gone from the phone");
        sms.apply(&body(json!({ "messages": [] })));
        assert_eq!(sms.conversations(10, &no_names).len(), 2);
    }

    #[test]
    fn a_thread_reply_merges_by_id_and_sorts_by_date() {
        let mut sms = Sms::default();
        sms.apply(&digest());
        let reply = body(json!({ "messages": [
            { "_id": 899, "thread_id": 42, "addresses": [ { "address": "+15550100" } ], "body": "Where?", "date": 1761252900000i64, "type": 2, "read": 1 },
            { "_id": 900, "thread_id": 42, "addresses": [ { "address": "+15550100" } ], "body": "See you at 8", "date": 1761253000000i64, "type": 1, "read": 1 },
            { "_id": 898, "thread_id": 42, "addresses": [ { "address": "+15550100" } ], "body": "Hi", "date": 1761252800000i64, "type": 1, "read": 1 } ] }));
        let changes = sms.apply(&reply);
        assert_eq!(changes.threads, vec![42]);
        let messages = sms.messages(42, 10, &no_names);
        let ids: Vec<i64> = messages.iter().map(|m| m["id"].as_i64().unwrap()).collect();
        assert_eq!(ids, vec![898, 899, 900]);
        assert_eq!(messages[1]["outgoing"], true);
        assert_eq!(messages[2]["read"], true);
        assert_eq!(sms.messages(42, 2, &no_names).len(), 2);
        assert_eq!(sms.messages(42, 2, &no_names)[0]["id"], 899, "the newest of them");
        // The same reply again changes no conversation
        assert!(!sms.apply(&reply).conversations);
        assert!(sms.messages(1234, 10, &no_names).is_empty());
    }

    #[test]
    fn a_new_message_pushed_by_the_phone() {
        let mut sms = Sms::default();
        sms.apply(&digest());
        let push = body(json!({ "version": 2, "messages": [
            { "_id": 901, "thread_id": 42, "addresses": [ { "address": "+15550100" }, { "address": "insert-address-token" } ], "body": "", "date": 1761253100000i64, "type": 1, "read": 0, "event": 1, "sub_id": 1,
              "attachments": [ { "part_id": 190, "mime_type": "image/jpeg", "encoded_thumbnail": "iVBORw0KGgo=", "unique_identifier": "PART_1" } ] } ] }));
        let changes = sms.apply(&push);
        assert!(changes.conversations);
        let list = sms.conversations(10, &no_names);
        assert_eq!(list[0]["snippet"], text::tr("Attachment"));
        assert_eq!(list[0]["addresses"], json!(["+15550100"]), "the placeholder is removed");
        assert_eq!(list[0]["group"], false);
    }

    #[test]
    fn reading_on_the_phone_reads_the_thread() {
        let mut sms = Sms::default();
        sms.apply(&digest());
        assert_eq!(sms.conversations(10, &no_names)[0]["unread"], true);
        sms.apply(&body(json!({ "messages": [ { "_id": 900, "thread_id": 42, "addresses": [ { "address": "+15550100" } ], "body": "See you at 8", "date": 1761253000000i64, "type": 1, "read": 1 } ] })));
        assert_eq!(sms.conversations(10, &no_names)[0]["unread"], false);
    }

    #[test]
    fn what_is_not_a_message_is_skipped() {
        let mut sms = Sms::default();
        let changes = sms.apply(&body(json!({ "messages": [
            { "_id": 1, "thread_id": 1, "addresses": [ { "address": "555" } ], "body": "draft", "date": 1, "type": 3 },
            { "_id": 2, "thread_id": 1, "addresses": [ { "address": "555" } ], "body": "all", "date": 1, "type": 0 },
            { "_id": 3, "thread_id": 1, "addresses": [], "body": "nobody", "date": 1, "type": 1 },
            { "_id": 4, "thread_id": 1, "addresses": [ { "address": "insert-address-token" } ], "body": "placeholder", "date": 1, "type": 1 },
            { "_id": 5, "addresses": [ { "address": "555" } ], "body": "no thread", "date": 1, "type": 1 },
            { "_id": 6, "thread_id": 1, "addresses": "555", "body": "bad addresses", "date": 1, "type": 1 },
            "not an object", 7, null ] })));
        assert_eq!(changes, Changes::default());
        assert!(sms.is_empty());
    }

    #[test]
    fn names_come_from_the_contacts_and_groups_say_so() {
        let mut sms = Sms::default();
        sms.apply(&body(json!({ "messages": [
            { "_id": 1, "thread_id": 5, "addresses": [ { "address": "+15550100" }, { "address": "+15550101" }, { "address": "+15550102" } ], "body": "group", "date": 9, "type": 1, "read": 0 } ] })));
        let names = |a: &str| (a == "+15550100").then(|| "Alex".to_string());
        let c = &sms.conversations(10, &names)[0];
        assert_eq!(c["group"], true);
        assert_eq!(c["name"], text::trf("{name} and {count} more", &[("name", "Alex"), ("count", "2")]));
        assert_eq!(sms.messages(5, 10, &names)[0]["name"], "Alex");
        assert_eq!(sms.participants(5).unwrap().len(), 3);
    }

    #[test]
    fn the_threads_and_messages_kept_are_bounded() {
        let mut sms = Sms::default();
        let messages: Vec<Value> = (0..limits::MESSAGES_PER_THREAD as i64 + 20).map(|i| json!({ "_id": i, "thread_id": 1, "addresses": [ { "address": "555" } ], "body": "m", "date": i, "type": 1, "read": 1 })).collect();
        sms.apply(&body(json!({ "messages": messages })));
        assert_eq!(sms.messages(1, 10_000, &no_names).len(), limits::MESSAGES_PER_THREAD);
        assert_eq!(sms.messages(1, 1, &no_names)[0]["id"], limits::MESSAGES_PER_THREAD as i64 + 19);
    }

    #[test]
    fn a_packet_of_forty_thousand_threads_is_cheap_and_bounded() {
        let mut sms = Sms::default();
        let messages: Vec<Value> = (0..40_000i64).map(|i| json!({ "_id": i, "thread_id": i, "addresses": [ { "address": "555" } ], "body": "m", "date": i, "type": 1, "read": 0 })).collect();
        let started = std::time::Instant::now();
        let changes = sms.apply(&body(json!({ "messages": messages })));
        assert!(started.elapsed().as_secs() < 3, "{:?}", started.elapsed());
        assert!(changes.threads.len() <= THREAD_SIGNALS, "{} signals", changes.threads.len());
        assert!(changes.conversations);
        assert!(sms.conversations(10_000, &no_names).len() <= limits::THREADS_KEPT);
        // Several packets do not add up to more than may be kept
        for round in 0..5i64 {
            let more: Vec<Value> = (0..5_000i64).map(|i| json!({ "_id": round * 10_000 + i, "thread_id": 100_000 + round * 10_000 + i, "addresses": [ { "address": "555" } ], "body": "m", "date": 100_000 + i, "type": 1 })).collect();
            sms.apply(&body(json!({ "messages": more })));
        }
        assert!(sms.threads.len() <= limits::THREADS_KEPT);
    }

    #[test]
    fn the_texts_of_a_phone_have_a_budget() {
        // The budget is a million bytes here, to keep the test short; the rule is the same at thirty-two million
        let mut sms = Sms { budget: 1024 * 1024, ..Sms::default() };
        let long = "x".repeat(limits::MESSAGE_BODY);
        for thread in 0..40i64 {
            let messages: Vec<Value> = (0..20i64).map(|i| json!({ "_id": thread * 1000 + i, "thread_id": thread, "addresses": [ { "address": "555" } ], "body": long, "date": thread * 1000 + i, "type": 1 })).collect();
            sms.apply(&body(json!({ "messages": messages })));
            assert!(sms.bytes <= sms.budget, "{} bytes after thread {thread}", sms.bytes);
        }
        let kept = sms.conversations(10_000, &no_names);
        assert!(!kept.is_empty() && kept.len() < 40, "{} conversations kept", kept.len());
        assert_eq!(kept[0]["thread_id"], 39, "the newest conversations stay, the oldest go");
        // The accounting is exact: dropping everything leaves nothing
        let threads: Vec<i64> = sms.threads.keys().copied().collect();
        for t in threads {
            sms.drop_thread(t);
        }
        assert_eq!(sms.bytes, 0);
    }

    #[test]
    fn addresses_are_bounded() {
        let mut sms = Sms::default();
        let many: Vec<Value> = (0..100).map(|i| json!({ "address": format!("{}{i}", "9".repeat(200)) })).collect();
        sms.apply(&body(json!({ "messages": [ { "_id": 1, "thread_id": 1, "addresses": many, "body": "b", "date": 1, "type": 1 } ] })));
        let addresses = sms.participants(1).unwrap();
        assert_eq!(addresses.len(), ADDRESSES_PER_MESSAGE);
        assert!(addresses.iter().all(|a| a.chars().count() <= ADDRESS_LENGTH));
    }

    #[test]
    fn only_a_text_that_came_in_just_now_is_fresh() {
        let now = 1761253100000i64;
        let text = |id: i64, date: i64, kind: i32, read: i32, body: &str| json!({ "_id": id, "thread_id": 42, "addresses": [ { "address": "+15550100" } ], "body": body, "date": date, "type": kind, "read": read });
        let mut sms = Sms::default();
        // Nothing is handed back unless asked
        assert!(sms.apply(&body(json!({ "messages": [ text(1, now, 1, 0, "code is 482913") ] }))).fresh.is_empty());
        let mut sms = Sms::default();
        let fresh = sms.apply_looking(&body(json!({ "messages": [ text(1, now - 30_000, 1, 0, "new") ] })), Some(now)).fresh;
        assert_eq!(fresh, vec!["new".to_string()]);
        // The same text again, an old one, one of ours, one read already and one from the future are none
        let again = body(json!({ "messages": [
            text(1, now - 30_000, 1, 0, "new"),
            text(2, now - 10 * 60_000, 1, 0, "old"),
            text(3, now, 2, 1, "ours"),
            text(4, now, 1, 1, "read"),
            text(5, now + 10 * 60_000, 1, 0, "future") ] }));
        assert!(sms.apply_looking(&again, Some(now)).fresh.is_empty());
    }

    #[test]
    fn the_requests() {
        assert_eq!(thread_request(42, 0, 0), json!({ "threadID": 42 }));
        assert_eq!(thread_request(42, 1761000000000, 10), json!({ "threadID": 42, "rangeStartTimestamp": 1761000000000i64, "numberToRequest": 10 }));
        let to = vec!["+15550100".to_string()];
        assert_eq!(send_request(&to, "Running late, start without me.", true), json!({ "version": 2, "addresses": [ { "address": "+15550100" } ], "messageBody": "Running late, start without me." }));
        assert_eq!(send_request(&to, "Running late, start without me.", false), json!({ "sendSms": true, "phoneNumber": "+15550100", "messageBody": "Running late, start without me." }));
    }
}
