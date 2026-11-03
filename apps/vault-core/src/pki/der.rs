//! The little DER the certificates need that the openssl crate has no safe way to build. Its extension builders join
//! values into OpenSSL's config strings, where a comma in a typed address becomes a second name, so what a person
//! typed only ever reaches a certificate through here.

pub const SEQUENCE: u8 = 0x30;
pub const SET: u8 = 0x31;
pub const INTEGER: u8 = 0x02;
pub const OCTET_STRING: u8 = 0x04;
pub const NULL: u8 = 0x05;
pub const OID: u8 = 0x06;
pub const UTC_TIME: u8 = 0x17;
pub const GENERALIZED_TIME: u8 = 0x18;
/// `[0]`, as a constructed context tag
pub const CONTEXT_0: u8 = 0xa0;

pub fn tlv(tag: u8, parts: &[&[u8]]) -> Vec<u8> {
    let len: usize = parts.iter().map(|part| part.len()).sum();
    let mut out = Vec::with_capacity(len + 5);
    out.push(tag);
    match len {
        0..=0x7f => out.push(len as u8),
        0x80..=0xff => out.extend([0x81, len as u8]),
        0x100..=0xffff => out.extend([0x82, (len >> 8) as u8, len as u8]),
        _ => out.extend([0x83, (len >> 16) as u8, (len >> 8) as u8, len as u8]),
    }
    for part in parts {
        out.extend_from_slice(part);
    }
    out
}

pub fn seq(parts: &[&[u8]]) -> Vec<u8> {
    tlv(SEQUENCE, parts)
}

pub fn octets(bytes: &[u8]) -> Vec<u8> {
    tlv(OCTET_STRING, &[bytes])
}

pub fn null() -> Vec<u8> {
    tlv(NULL, &[])
}

/// A dotted object identifier the code itself names, so it is well formed
pub fn oid(dotted: &str) -> Vec<u8> {
    let mut arcs = dotted.split('.').map(|arc| arc.parse::<u64>().unwrap_or(0));
    let first = arcs.next().unwrap_or(0) * 40 + arcs.next().unwrap_or(0);
    let mut body = Vec::new();
    for arc in std::iter::once(first).chain(arcs) {
        let mut groups = vec![(arc & 0x7f) as u8];
        let mut rest = arc >> 7;
        while rest > 0 {
            groups.push((rest & 0x7f) as u8 | 0x80);
            rest >>= 7;
        }
        groups.reverse();
        body.extend(groups);
    }
    tlv(OID, &[&body])
}

/// A positive integer from its big-endian bytes, with the zero in front that keeps its top bit from reading as a sign
pub fn uint(bytes: &[u8]) -> Vec<u8> {
    let start = bytes.iter().position(|byte| *byte != 0).unwrap_or(bytes.len().saturating_sub(1));
    let bytes = &bytes[start..];
    if bytes.first().is_some_and(|byte| byte & 0x80 != 0) {
        tlv(INTEGER, &[&[0], bytes])
    } else {
        tlv(INTEGER, &[bytes])
    }
}

/// The first element of `bytes`: its tag, its contents and what follows it. `None` on anything short: this only reads
/// what OpenSSL itself wrote, or a certificate OpenSSL has parsed.
pub fn read(bytes: &[u8]) -> Option<(u8, &[u8], &[u8])> {
    let (&tag, rest) = bytes.split_first()?;
    let (&first, rest) = rest.split_first()?;
    let (len, rest) = if first < 0x80 {
        (usize::from(first), rest)
    } else {
        let count = usize::from(first & 0x7f);
        if count == 0 || count > 3 || rest.len() < count {
            return None;
        }
        let (bytes, rest) = rest.split_at(count);
        (bytes.iter().fold(0usize, |len, byte| (len << 8) | usize::from(*byte)), rest)
    };
    if len > rest.len() {
        return None;
    }
    let (contents, rest) = rest.split_at(len);
    Some((tag, contents, rest))
}

/// A DER SET OF: its members in the order of their encodings, which a verifier that encodes it again depends on
pub fn set_of(mut members: Vec<Vec<u8>>) -> Vec<u8> {
    members.sort();
    let parts: Vec<&[u8]> = members.iter().map(Vec::as_slice).collect();
    tlv(SET, &parts)
}

/// A UTCTime up to 2049 and a GeneralizedTime from 2050, as RFC 5652 asks of `signingTime`
pub fn time(unix: i64) -> Vec<u8> {
    let days = unix.div_euclid(86400);
    let secs = unix.rem_euclid(86400);
    // Howard Hinnant's civil_from_days
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    let rest = format!("{month:02}{day:02}{:02}{:02}{:02}Z", secs / 3600, secs / 60 % 60, secs % 60);
    if (1950..2050).contains(&year) {
        tlv(UTC_TIME, &[format!("{:02}{rest}", year % 100).as_bytes()])
    } else {
        tlv(GENERALIZED_TIME, &[format!("{year:04}{rest}").as_bytes()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_encodings() {
        assert_eq!(oid("2.5.29.30"), [0x06, 0x03, 0x55, 0x1d, 0x1e]);
        assert_eq!(oid("1.3.6.1.4.1.311.10.3.12"), [0x06, 0x0a, 0x2b, 0x06, 0x01, 0x04, 0x01, 0x82, 0x37, 0x0a, 0x03, 0x0c]);
        assert_eq!(uint(&[0x80]), [0x02, 0x02, 0x00, 0x80]);
        assert_eq!(uint(&[0x00, 0x00, 0x05]), [0x02, 0x01, 0x05]);
        assert_eq!(tlv(0x30, &[&[0u8; 300]])[..4], [0x30, 0x82, 0x01, 0x2c]);
        assert_eq!(time(0), tlv(UTC_TIME, &[b"700101000000Z"]));
        assert_eq!(time(1793577600), tlv(UTC_TIME, &[b"261102000000Z"]));
        assert_eq!(time(2524608000), tlv(GENERALIZED_TIME, &[b"20500101000000Z"]));
    }
}
