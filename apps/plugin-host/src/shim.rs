//! `amethystora-plugin-host shim SOCKET -- COMMAND...`: runs inside a plugin's sandbox when it holds a network permission.
//! The sandbox's network namespace has only loopback, so HTTPS_PROXY points at 127.0.0.1 here; this forwards each
//! connection, byte for byte, to the host's proxy socket (the one thing bound into the sandbox), and ends with the plugin.

use std::io::Read;
use std::net::{Shutdown, TcpListener, TcpStream};
use std::os::unix::net::UnixStream;
use std::process::Command;

use crate::sandbox::SHIM_PORT;

fn pipe(mut from: impl Read, mut to: impl std::io::Write) {
    let _ = std::io::copy(&mut from, &mut to);
}

fn forward(client: TcpStream, socket: String) {
    let Ok(upstream) = UnixStream::connect(&socket) else { return };
    let (Ok(client_in), Ok(upstream_in)) = (client.try_clone(), upstream.try_clone()) else { return };
    std::thread::spawn(move || {
        pipe(client_in, &upstream);
        let _ = upstream.shutdown(Shutdown::Write);
    });
    pipe(upstream_in, &client);
    let _ = client.shutdown(Shutdown::Both);
}

/// Binds the port, then starts the plugin and waits for it: its exit code is ours
pub fn run(socket: &str, command: &[String]) -> i32 {
    let Some(program) = command.first() else { return 64 };
    let listener = match TcpListener::bind(("127.0.0.1", SHIM_PORT)) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("amethystora-plugin-host shim: {e}");
            return 1;
        }
    };
    let socket = socket.to_string();
    std::thread::spawn(move || {
        for client in listener.incoming().flatten() {
            let socket = socket.clone();
            std::thread::spawn(move || forward(client, socket));
        }
    });
    match Command::new(program).args(&command[1..]).status() {
        Ok(status) => status.code().unwrap_or(1),
        Err(e) => {
            eprintln!("amethystora-plugin-host shim: {e}");
            127
        }
    }
}
