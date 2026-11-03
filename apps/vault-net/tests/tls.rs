//! TLS as the network service does it: the machine's own trust, a private CA trusted for one connection by the fingerprint
//! the person saw, and nothing else. A server of the test's own, with certificates it makes.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread::JoinHandle;

use openssl::asn1::Asn1Time;
use openssl::bn::{BigNum, MsbOption};
use openssl::hash::MessageDigest;
use openssl::pkey::{PKey, Private};
use openssl::rsa::Rsa;
use openssl::ssl::{SslAcceptor, SslMethod};
use openssl::x509::extension::{BasicConstraints, KeyUsage, SubjectAlternativeName};
use openssl::x509::{X509Builder, X509NameBuilder, X509};

use amethystora_vault_net::http::{fingerprint, probe, Failure, Session};

fn name(common: &str) -> openssl::x509::X509Name {
    let mut name = X509NameBuilder::new().unwrap();
    name.append_entry_by_text("CN", common).unwrap();
    name.build()
}

fn serial() -> openssl::asn1::Asn1Integer {
    let mut number = BigNum::new().unwrap();
    number.rand(64, MsbOption::MAYBE_ZERO, false).unwrap();
    number.to_asn1_integer().unwrap()
}

/// A certificate for `common` (and `hosts` as alternative names), signed by `issuer` or by its own key
fn certificate(common: &str, hosts: &[&str], ca: bool, issuer: Option<(&X509, &PKey<Private>)>) -> (X509, PKey<Private>) {
    let key = PKey::from_rsa(Rsa::generate(2048).unwrap()).unwrap();
    let mut builder = X509Builder::new().unwrap();
    builder.set_version(2).unwrap();
    builder.set_serial_number(&serial()).unwrap();
    builder.set_subject_name(&name(common)).unwrap();
    builder.set_pubkey(&key).unwrap();
    builder.set_not_before(&Asn1Time::days_from_now(0).unwrap()).unwrap();
    builder.set_not_after(&Asn1Time::days_from_now(2).unwrap()).unwrap();
    if ca {
        builder.append_extension(BasicConstraints::new().critical().ca().build().unwrap()).unwrap();
        builder.append_extension(KeyUsage::new().critical().key_cert_sign().build().unwrap()).unwrap();
    }
    if !hosts.is_empty() {
        let mut names = SubjectAlternativeName::new();
        for host in hosts {
            if host.parse::<std::net::IpAddr>().is_ok() {
                names.ip(host);
            } else {
                names.dns(host);
            }
        }
        let extension = names.build(&builder.x509v3_context(issuer.map(|(cert, _)| cert.as_ref()), None)).unwrap();
        builder.append_extension(extension).unwrap();
    }
    match issuer {
        Some((cert, signing)) => {
            builder.set_issuer_name(cert.subject_name()).unwrap();
            builder.sign(signing, MessageDigest::sha256()).unwrap();
        }
        None => {
            builder.set_issuer_name(&name(common)).unwrap();
            builder.sign(&key, MessageDigest::sha256()).unwrap();
        }
    }
    (builder.build(), key)
}

/// A server that answers every request on the connections it accepts with `answer`, until it has served `count` requests
fn serve(cert: &X509, key: &PKey<Private>, chain: &[&X509], answer: &'static str, count: usize) -> (u16, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let mut acceptor = SslAcceptor::mozilla_intermediate_v5(SslMethod::tls()).unwrap();
    acceptor.set_private_key(key).unwrap();
    acceptor.set_certificate(cert).unwrap();
    for extra in chain {
        acceptor.add_extra_chain_cert((*extra).clone()).unwrap();
    }
    let acceptor = acceptor.build();
    let thread = std::thread::spawn(move || {
        for _ in 0..count {
            let Ok((tcp, _)) = listener.accept() else { return };
            let Ok(mut tls) = acceptor.accept(tcp) else { continue };
            let mut seen = Vec::new();
            let mut chunk = [0u8; 1024];
            while !seen.windows(4).any(|w| w == b"\r\n\r\n") {
                match tls.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => seen.extend_from_slice(&chunk[..n]),
                }
            }
            let _ = tls.write_all(answer.as_bytes());
        }
    });
    (port, thread)
}

const OK: &str = "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: 2\r\n\r\nok";

fn pem(cert: &X509) -> String {
    String::from_utf8(cert.to_pem().unwrap()).unwrap()
}

fn get(session: &mut Session, port: u16) -> Result<u16, Failure> {
    session.request("GET", &format!("https://localhost:{port}/api/sync"), &[], b"").map(|(status, _)| status)
}

#[test]
fn a_private_ca_is_trusted_by_the_fingerprint_the_person_saw_and_for_this_connection_only() {
    let (ca, ca_key) = certificate("Test CA", &[], true, None);
    let (leaf, leaf_key) = certificate("localhost", &["localhost", "127.0.0.1"], false, Some((&ca, &ca_key)));
    let (port, thread) = serve(&leaf, &leaf_key, &[], OK, 3);
    let origin = format!("https://localhost:{port}");
    // Without it, the machine's own trust does not know this CA
    assert_eq!(get(&mut Session::new(vec![origin.clone()], None, None).unwrap(), port), Err(Failure::Untrusted));
    // With it, and the fingerprint as shown
    let shown = fingerprint(&ca);
    assert_eq!(shown.len(), 64);
    let mut session = Session::new(vec![origin.clone()], Some(&pem(&ca)), Some(&shown)).unwrap();
    assert_eq!(get(&mut session, port), Ok(200));
    // Another fingerprint than the certificate's, and a CA without one, are refused before any connection
    assert_eq!(Session::new(vec![origin.clone()], Some(&pem(&ca)), Some(&"0".repeat(64))).err(), Some(Failure::Untrusted));
    assert_eq!(Session::new(vec![origin.clone()], Some(&pem(&ca)), None).err(), Some(Failure::Untrusted));
    assert_eq!(Session::new(vec![origin], None, Some(&shown)).err(), Some(Failure::Untrusted));
    drop(thread);
}

#[test]
fn the_name_has_to_be_the_servers() {
    let (ca, ca_key) = certificate("Test CA", &[], true, None);
    let (leaf, leaf_key) = certificate("elsewhere.test", &["elsewhere.test"], false, Some((&ca, &ca_key)));
    let (port, _thread) = serve(&leaf, &leaf_key, &[], OK, 1);
    let mut session = Session::new(vec![format!("https://localhost:{port}")], Some(&pem(&ca)), Some(&fingerprint(&ca))).unwrap();
    assert_eq!(get(&mut session, port), Err(Failure::Untrusted));
}

#[test]
fn a_pinned_server_certificate_is_trusted_itself() {
    let (leaf, key) = certificate("localhost", &["localhost"], false, None);
    let (port, _thread) = serve(&leaf, &key, &[], OK, 1);
    let mut session = Session::new(vec![format!("https://localhost:{port}")], Some(&pem(&leaf)), Some(&fingerprint(&leaf))).unwrap();
    assert_eq!(get(&mut session, port), Ok(200));
}

#[test]
fn the_probe_shows_what_the_server_shows_without_trusting_it() {
    let (ca, ca_key) = certificate("Test CA", &[], true, None);
    let (leaf, leaf_key) = certificate("localhost", &["localhost"], false, Some((&ca, &ca_key)));
    let (port, _thread) = serve(&leaf, &leaf_key, &[&ca], OK, 1);
    let seen = probe(&format!("https://localhost:{port}")).unwrap();
    assert!(!seen.trusted, "the machine does not know this CA");
    assert_eq!(seen.chain.len(), 2);
    let (subject, issuer, sha256, pem_text, self_signed) = &seen.chain[0];
    assert!(subject.contains("localhost") && issuer.contains("Test CA") && !self_signed);
    assert_eq!(*sha256, fingerprint(&leaf));
    assert!(pem_text.starts_with("-----BEGIN CERTIFICATE-----"));
    let (_, _, ca_sha, _, ca_self) = &seen.chain[1];
    assert_eq!(*ca_sha, fingerprint(&ca));
    assert!(*ca_self);
    // Plain http is shown as what it is: nothing to trust
    assert!(probe("http://127.0.0.1:1").unwrap().chain.is_empty());
}

#[test]
fn a_redirect_off_the_host_is_handed_back_and_never_followed() {
    let (leaf, key) = certificate("localhost", &["localhost"], false, None);
    let (port, _thread) = serve(&leaf, &key, &[], "HTTP/1.1 302 Found\r\nLocation: https://elsewhere.test/\r\nConnection: close\r\nContent-Length: 0\r\n\r\n", 1);
    let mut session = Session::new(vec![format!("https://localhost:{port}")], Some(&pem(&leaf)), Some(&fingerprint(&leaf))).unwrap();
    assert_eq!(get(&mut session, port), Ok(302));
}

#[test]
fn the_socket_protocol_needs_a_session_and_keeps_to_its_origins() {
    use amethystora_vault_net::serve as serve_lines;
    let script = [
        r#"{"v":1,"op":"request","method":"GET","url":"https://localhost:9/x","headers":[],"body":""}"#,
        r#"{"v":1,"op":"session","origins":["https://localhost:9","https://other.test:443"],"ca":"x"}"#,
        r#"{"v":1,"op":"session","origins":["https://user@localhost:9"]}"#,
        r#"{"v":2,"op":"session","origins":["https://localhost:9"]}"#,
        r#"{"v":1,"op":"session","origins":["https://localhost:9"]}"#,
        r#"{"v":1,"op":"request","method":"GET","url":"https://elsewhere.test/x","headers":[],"body":""}"#,
        r#"{"v":1,"op":"request","method":"DELETE","url":"https://localhost:9/x","headers":[],"body":""}"#,
        r#"{"v":1,"op":"request","method":"GET","url":"https://localhost:9/x","headers":[["Host","evil"]],"body":""}"#,
        r#"not json"#,
    ]
    .join("\n")
        + "\n";
    let mut reader = std::io::BufReader::new(script.as_bytes());
    let mut written = Vec::new();
    serve_lines(&mut reader, &mut written, &|origins, ca, pin| Session::new(origins, ca, pin));
    let answers: Vec<serde_json::Value> = String::from_utf8(written).unwrap().lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    assert_eq!(answers.len(), 9);
    let why = |at: usize| answers[at]["why"].as_str().map(str::to_string);
    assert_eq!(why(0).as_deref(), Some("not-allowed"), "no session yet");
    assert_eq!(why(1).as_deref(), Some("untrusted"), "a CA without its pin");
    assert_eq!(why(2).as_deref(), Some("not-allowed"), "a user name in an origin");
    assert_eq!(why(3).as_deref(), Some("not-allowed"), "another version");
    assert_eq!(answers[4]["ok"], true);
    assert_eq!(why(5).as_deref(), Some("not-allowed"), "a host outside the session");
    assert_eq!(why(6).as_deref(), Some("not-allowed"), "a method that destroys");
    assert_eq!(why(7).as_deref(), Some("not-allowed"), "a header that is the connection's own");
    assert_eq!(why(8).as_deref(), Some("not-allowed"));
}

#[test]
fn a_line_past_the_limit_is_refused_and_ends_the_connection() {
    use amethystora_vault_net::{serve as serve_lines, MOST_REQUEST};
    let mut script = vec![b'x'; MOST_REQUEST + 10];
    script.push(b'\n');
    let mut reader = std::io::BufReader::new(&script[..]);
    let mut written = Vec::new();
    serve_lines(&mut reader, &mut written, &|origins, ca, pin| Session::new(origins, ca, pin));
    let answer: serde_json::Value = serde_json::from_str(String::from_utf8(written).unwrap().lines().next().unwrap()).unwrap();
    assert_eq!(answer["why"], "too-large");
}

// --- The review's L1 and L2 ---------------------------------------------------------------------------------------------------

/// A TLS server that answers once and then ends the connection with no close_notify, as anyone on the network can make it
fn cut_short(answer: &'static [u8]) -> (Session, String, JoinHandle<()>) {
    let (cert, key) = certificate("localhost", &["localhost"], false, None);
    let mut acceptor = SslAcceptor::mozilla_intermediate_v5(SslMethod::tls()).unwrap();
    acceptor.set_private_key(&key).unwrap();
    acceptor.set_certificate(&cert).unwrap();
    let acceptor = acceptor.build();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("https://localhost:{}", listener.local_addr().unwrap().port());
    let session = Session::new(vec![origin.clone()], Some(&pem(&cert)), Some(&fingerprint(&cert))).unwrap();
    let thread = std::thread::spawn(move || {
        let (tcp, _) = listener.accept().unwrap();
        let mut tls = acceptor.accept(tcp).unwrap();
        let mut got = [0u8; 4096];
        let _ = tls.read(&mut got).unwrap();
        tls.write_all(answer).unwrap();
        tls.flush().unwrap();
        let _ = tls.get_ref().shutdown(std::net::Shutdown::Both);
        std::mem::forget(tls);
    });
    (session, origin, thread)
}

#[test]
fn a_body_that_ends_with_the_connection_and_no_close_notify_is_not_whole() {
    for (name, answer) in [
        ("until close", &b"HTTP/1.1 200 OK\r\n\r\n{\"ciphers\":[]}"[..]),
        ("by its length", &b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\n{\"ciphers\":[]}"[..]),
        ("in chunks", &b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\n{}\r\n"[..]),
    ] {
        let (mut session, origin, thread) = cut_short(answer);
        let result = session.request("GET", &format!("{origin}/api/sync"), &[], b"");
        assert!(result.is_err(), "{name}: {:?}", result.map(|(status, body)| (status, body.len())));
        thread.join().unwrap();
    }
}

#[test]
fn a_chunk_size_that_would_wrap_the_limit_is_refused() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut got = [0u8; 4096];
        let _ = socket.read(&mut got);
        let _ = socket.write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n1\r\na\r\nFFFFFFFFFFFFFFFF\r\n");
        let block = vec![b'x'; 1024 * 1024];
        for _ in 0..64 {
            if socket.write_all(&block).is_err() {
                break;
            }
        }
    });
    let origin = format!("http://127.0.0.1:{port}");
    let mut session = Session::new(vec![origin.clone()], None, None).unwrap();
    assert_eq!(session.request("GET", &format!("{origin}/x"), &[], b"").err(), Some(Failure::TooLarge));
}
