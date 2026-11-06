//! Snap with your phone (R7-69), from `plans/amethystora-connect/spec-photo.md`: the computer asks the phone to open its camera, and
//! the picture arrives as a file.
//!
//! **A current KDE Connect app has no such request** (the upstream plugin was removed in 2023), so this exists for the phone whose
//! identity lists `kdeconnect.photo.request` among what it takes, and for no other: the feature "photo" is offered only then, and only
//! while the switch `photo` is on (off, it is not even listed in this computer's capabilities). The rules, from the spec:
//!
//! - the request is made only by the person pressing a button in the Phone app (the bus lets only the app call `RequestPhoto`);
//! - one request waits per phone, 120 seconds, and a picture that arrives with none waiting is refused (its payload is never fetched);
//! - the request has no path in it: the file goes where every picture from a phone goes, under a name that cannot leave it,
//!   never over a file, at most 64 MiB;
//! - a cancel from the phone drops the request;
//! - nothing opens by itself, and the program says "asked", never that the camera is open.

use std::rc::Rc;
use std::time::Instant;

use serde_json::json;

use crate::dbus;
use crate::device::Device;
use crate::limits;
use crate::link::Link;
use crate::notify::Notice;
use crate::packet::Packet;
use crate::service::{ApiError, ApiResult, Service};
use crate::text::{self, tr, trf};

pub const REQUEST: &str = "kdeconnect.photo.request";
pub const PHOTO: &str = "kdeconnect.photo";

/// The photo asked of one phone: which request it is (a later one is not answered by an earlier one's timer), and since when
#[derive(Debug, Default, Clone, Copy)]
pub struct Pending {
    round: u64,
    since: Option<Instant>,
}

impl Pending {
    pub fn waiting(&self) -> bool {
        self.since.is_some_and(|t| t.elapsed() < limits::PHOTO_WAIT)
    }
}

/// What to do with a `kdeconnect.photo` packet
#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The person left the camera
    Cancel,
    /// A picture, with the name the phone gave it (not yet safe) and its modification time
    Take { name: String, modified: Option<i64>, size: u64 },
    /// Anything else: nothing is fetched
    Refuse,
}

/// Reads the packet. `waiting` is whether a request of ours waits for it: a picture with none is refused.
pub fn judge(p: &Packet, waiting: bool) -> Verdict {
    if p.bool("cancel") == Some(true) {
        return if waiting { Verdict::Cancel } else { Verdict::Refuse };
    }
    if !waiting {
        return Verdict::Refuse;
    }
    let (Some(name), Ok(Some(payload))) = (p.str("filename"), p.payload()) else { return Verdict::Refuse };
    // Not nothing, and not more than a photo is
    if payload.size == 0 || payload.size > limits::PHOTO_MAX {
        return Verdict::Refuse;
    }
    Verdict::Take { name: name.to_string(), modified: p.int("lastModified"), size: payload.size }
}

impl Service {
    /// The person pressed Snap with your phone. Fails when the switch is off, the phone does not take the request, one is
    /// already waiting, or the phone is away.
    pub fn request_photo(self: &Rc<Self>, id: &str) -> ApiResult<()> {
        if !self.photo.get() {
            return Err(ApiError::unavailable("Snap with your phone is off"));
        }
        let dev = self.device(id).ok_or_else(|| ApiError::unavailable("no such device"))?;
        if !dev.features().contains(&"photo") {
            return Err(ApiError::unavailable("the phone does not take a photo request"));
        }
        if dev.st.borrow().photo.waiting() {
            return Err(ApiError::unavailable("a photo is already asked for"));
        }
        if !self.send_to(&dev, REQUEST, json!({})) {
            return Err(ApiError::unavailable("the phone cannot be asked now"));
        }
        let round = self.next_id();
        dev.st.borrow_mut().photo = Pending { round, since: Some(Instant::now()) };
        dbus::emit(self, Some(&dev), "Photo", Some(("asked",).to_variant()));
        self.raise(Notice {
            key: format!("photo:{}:{round}", dev.id),
            summary: trf("Asked “{name}” to open its camera", &[("name", &dev.name())]),
            body: tr("Take the picture on the phone. Cancel here forgets the request; it cannot close the camera on the phone."),
            icon: Some("camera-photo-symbolic".into()),
            actions: vec![("cancel".to_string(), tr("Cancel"))],
            timeout_ms: limits::PHOTO_WAIT.as_millis() as i32,
            ..Notice::default()
        });
        let service = self.clone();
        let device = dev.id.clone();
        self.spawn_task(async move {
            glib::timeout_future(limits::PHOTO_WAIT).await;
            if let Some(dev) = service.device(&device) {
                let waiting = {
                    let st = dev.st.borrow();
                    st.photo.round == round && st.photo.since.is_some()
                };
                if waiting {
                    dev.st.borrow_mut().photo = Pending::default();
                    service.notifier.close(&format!("photo:{device}:{round}"));
                    service.raise(Notice {
                        key: format!("photo-end:{device}"),
                        summary: tr("No photo came"),
                        body: trf("“{name}” did not send a picture.", &[("name", &dev.name())]),
                        icon: Some("camera-photo-symbolic".into()),
                        ..Notice::default()
                    });
                    dbus::emit(&service, Some(&dev), "Photo", Some(("timeout",).to_variant()));
                }
            }
        });
        Ok(())
    }

    /// Cancel on the card: forgets the request here (the phone's camera screen cannot be closed from here)
    pub fn cancel_photo(self: &Rc<Self>, dev: &Rc<Device>, rest: &str) {
        let same = rest.parse::<u64>().is_ok_and(|round| dev.st.borrow().photo.round == round);
        if same && dev.st.borrow().photo.since.is_some() {
            dev.st.borrow_mut().photo = Pending::default();
            self.notifier.close(&format!("photo:{}:{rest}", dev.id));
            dbus::emit(self, Some(dev), "Photo", Some(("cancelled",).to_variant()));
        }
    }

    pub(crate) fn on_photo(self: &Rc<Self>, dev: &Rc<Device>, link: &Rc<Link>, p: Packet) {
        let waiting = dev.st.borrow().photo.waiting();
        match judge(&p, waiting) {
            Verdict::Cancel => {
                let round = dev.st.borrow().photo.round;
                dev.st.borrow_mut().photo = Pending::default();
                self.notifier.close(&format!("photo:{}:{round}", dev.id));
                self.raise(Notice {
                    key: format!("photo-end:{}", dev.id),
                    summary: tr("The photo was cancelled on the phone"),
                    icon: Some("camera-photo-symbolic".into()),
                    ..Notice::default()
                });
                dbus::emit(self, Some(dev), "Photo", Some(("cancelled",).to_variant()));
            }
            Verdict::Take { name, modified, .. } => {
                let round = dev.st.borrow().photo.round;
                dev.st.borrow_mut().photo = Pending::default();
                self.notifier.close(&format!("photo:{}:{round}", dev.id));
                // The picture is a file like any other from a phone: fetched under a name that cannot leave its folder, never over
                // another, and said when it is in (with Open and Show in folder, never opened by itself)
                self.receive_file_now(dev, link, &p, text::file_name(&name), modified, None);
                dbus::emit(self, Some(dev), "Photo", Some(("received",).to_variant()));
            }
            Verdict::Refuse => self.refuse_payload(link, dev, p.payload().ok().flatten()),
        }
    }
}

use glib::variant::ToVariant;

#[cfg(test)]
mod tests {
    use super::*;

    fn packet(json: &str) -> Packet {
        Packet::parse(json.as_bytes()).unwrap()
    }

    const PICTURE: &str = r#"{"id":1,"type":"kdeconnect.photo","body":{"filename":"JPEG_20261106_141500_123456.jpg","lastModified":1730000100000},"payloadSize":1834201,"payloadTransferInfo":{"port":1741}}"#;
    const CANCEL: &str = r#"{"id":1,"type":"kdeconnect.photo","body":{"cancel":true}}"#;

    #[test]
    fn a_picture_that_was_asked_for_is_taken() {
        assert_eq!(judge(&packet(PICTURE), true), Verdict::Take { name: "JPEG_20261106_141500_123456.jpg".into(), modified: Some(1730000100000), size: 1834201 });
    }

    #[test]
    fn a_picture_or_a_cancel_nobody_asked_for_is_refused() {
        assert_eq!(judge(&packet(PICTURE), false), Verdict::Refuse);
        assert_eq!(judge(&packet(CANCEL), false), Verdict::Refuse);
    }

    #[test]
    fn a_cancel_drops_the_request() {
        assert_eq!(judge(&packet(CANCEL), true), Verdict::Cancel);
    }

    #[test]
    fn a_photo_of_no_size_or_too_large_or_with_no_payload_is_refused() {
        let too_big = PICTURE.replace("1834201", &(limits::PHOTO_MAX + 1).to_string());
        let empty = PICTURE.replace("1834201", "0");
        let no_payload = r#"{"id":1,"type":"kdeconnect.photo","body":{"filename":"a.jpg"}}"#;
        let no_name = r#"{"id":1,"type":"kdeconnect.photo","body":{},"payloadSize":5,"payloadTransferInfo":{"port":1741}}"#;
        let at_the_limit = PICTURE.replace("1834201", &limits::PHOTO_MAX.to_string());
        for refused in [&too_big, &empty] {
            assert_eq!(judge(&packet(refused), true), Verdict::Refuse);
        }
        assert_eq!(judge(&packet(no_payload), true), Verdict::Refuse);
        assert_eq!(judge(&packet(no_name), true), Verdict::Refuse);
        assert!(matches!(judge(&packet(&at_the_limit), true), Verdict::Take { .. }));
    }

    #[test]
    fn a_request_waits_a_while_and_then_does_not() {
        let mut pending = Pending::default();
        assert!(!pending.waiting());
        pending.since = Some(Instant::now());
        assert!(pending.waiting());
        pending.since = Instant::now().checked_sub(limits::PHOTO_WAIT + std::time::Duration::from_secs(1));
        assert!(!pending.waiting(), "an answer after the time is not an answer");
    }
}
