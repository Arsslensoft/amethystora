//! The network service's own binary, started as systemd would start it: each connection on a thread of its own, so that one
//! account's slow server never holds up another account's session (the review's M7).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::os::fd::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::Duration;

fn line(stream: &mut UnixStream, text: &str) -> String {
    stream.write_all(text.as_bytes()).unwrap();
    stream.write_all(b"\n").unwrap();
    let mut answer = String::new();
    BufReader::new(stream.try_clone().unwrap()).read_line(&mut answer).unwrap();
    answer
}

#[test]
fn a_slow_server_on_one_connection_does_not_hold_up_another() {
    let folder = std::env::temp_dir().join(format!("avn-threads-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let socket = folder.join("net.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    let fd = listener.as_raw_fd();
    // A shell makes the variable name the program's own number, which stays the same through exec
    let mut command = Command::new("/bin/sh");
    command.arg("-c").arg("export LISTEN_PID=$$ LISTEN_FDS=1; exec \"$0\"").arg(env!("CARGO_BIN_EXE_amethystora-vault-net")).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    // SAFETY: in the child, between fork and exec: the socket becomes descriptor 3
    unsafe {
        command.pre_exec(move || {
            // Descriptor 3, kept through the exec (a descriptor made by Rust is closed by it, and may be 3 already)
            let made = if fd == 3 { libc::fcntl(3, libc::F_SETFD, 0) } else { libc::dup2(fd, 3) };
            if made < 0 { Err(std::io::Error::last_os_error()) } else { Ok(()) }
        });
    }
    let mut child = command.spawn().unwrap();

    // A server that takes the request and never answers
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://127.0.0.1:{}", server.local_addr().unwrap().port());
    std::thread::spawn(move || {
        let mut held = Vec::new();
        for stream in server.incoming().flatten() {
            let mut stream = stream;
            let mut got = [0u8; 4096];
            let _ = stream.read(&mut got);
            held.push(stream);
        }
    });

    let session = format!("{{\"v\":1,\"op\":\"session\",\"origins\":[\"{origin}\"]}}");
    let mut slow = UnixStream::connect(&socket).unwrap();
    slow.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    assert!(line(&mut slow, &session).contains("\"ok\":true"));
    slow.write_all(format!("{{\"v\":1,\"op\":\"request\",\"method\":\"GET\",\"url\":\"{origin}/api/sync\",\"headers\":[],\"body\":\"\"}}\n").as_bytes()).unwrap();
    std::thread::sleep(Duration::from_millis(300));

    // Another connection is answered at once, while the first still waits
    let mut quick = UnixStream::connect(&socket).unwrap();
    quick.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    quick.write_all(format!("{session}\n").as_bytes()).unwrap();
    let mut answer = String::new();
    let read = BufReader::new(quick.try_clone().unwrap()).read_line(&mut answer);
    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&folder);
    assert!(read.is_ok() && answer.contains("\"ok\":true"), "the second connection waited for the first: {read:?} {answer:?}");
}
