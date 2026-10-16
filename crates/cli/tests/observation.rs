#[path = "observation/fixture.rs"]
mod fixture;
use fixture::Fixture;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
const ENABLED: &str = "[locus]\nenabled = true\nendpoint = 'http://127.0.0.1:0'\n";
fn response(f: &Fixture, name: &'static str, status: u16) -> std::process::Output {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let data = request(&mut socket);
        let request = String::from_utf8_lossy(&data);
        match name {
            "send" => {
                assert!(request.starts_with("POST /api/v1/messages"));
                assert!(request.contains("private-message"));
                assert!(request.contains("private-request"));
            }
            "poll" => {
                assert!(request.starts_with("GET /api/v1/inbox/private-person?since=0"));
            }
            "reply" => {
                assert!(request.starts_with("POST /api/v1/replies"));
                assert!(request.contains("Bearer private-capability"));
                assert!(request.contains("private-reply"));
            }
            _ => unreachable!(),
        }
        socket.write_all(format!("HTTP/1.1 {status} Fixture\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{{\"ok\":true}}").as_bytes()).unwrap();
    });
    let out = f.run(name, &base, true);
    server.join().unwrap();
    out
}
#[test]
fn buffered() {
    let f = Fixture::new();
    f.configure(ENABLED);
    for name in ["send", "poll", "reply"] {
        assert_eq!(response(&f, name, 201).status.code(), Some(0));
        assert_eq!(response(&f, name, 403).status.code(), Some(1));
    }
    assert_eq!(
        f.run("reply", "http://127.0.0.1:0", false).status.code(),
        Some(1)
    );
    assert_eq!(
        f.run("poll", "http://127.0.0.1:0", true).status.code(),
        Some(1)
    );
    let atoms = f.records();
    assert!(atoms.iter().all(|a| a["producer"] == "stim"));
    for edge in ["enter", "return"] {
        assert_eq!(
            atoms.iter().filter(|a| a["source"]["edge"] == edge).count(),
            8
        );
    }
    for name in ["send", "poll", "reply"] {
        assert!(atoms.iter().any(|a| a["context"]["stim.command"] == name));
    }
    assert!(
        atoms
            .iter()
            .all(|a| a["context"].get("stim.fault").is_none())
    );
    let text = serde_json::to_string(&atoms).unwrap();
    for private in [
        "private-message",
        "private-reply",
        "private-person",
        "private-request",
        "private-capability",
        "127.0.0.1",
    ] {
        assert!(!text.contains(private));
    }
}
#[test]
fn isolated() {
    let f = Fixture::new();
    let baseline = response(&f, "send", 201);
    f.configure("[locus]\nenabled = false\n");
    let off = response(&f, "send", 201);
    assert_eq!(
        (off.status.code(), off.stdout, off.stderr),
        (
            baseline.status.code(),
            baseline.stdout.clone(),
            baseline.stderr.clone()
        )
    );
    assert!(!f.seat().join("state/locus").exists());
    for text in [
        ENABLED,
        "[locus]\nenabled = 'private-setting'\n",
        "[locus]\nenabled = true\nendpoint = ''\n",
    ] {
        f.configure(text);
        let out = response(&f, "send", 201);
        assert_eq!(out.status.code(), baseline.status.code());
        assert_eq!(out.stdout, baseline.stdout);
        assert!(!out.stderr.is_empty());
        assert!(!String::from_utf8_lossy(&out.stderr).contains("private-setting"));
        assert_eq!(response(&f, "poll", 403).status.code(), Some(1));
    }
    let blocked = Fixture::new();
    blocked.configure(ENABLED);
    std::fs::write(blocked.seat().join("state"), "occupied").unwrap();
    let out = response(&blocked, "send", 201);
    assert_eq!(out.status.code(), baseline.status.code());
    assert_eq!(out.stdout, baseline.stdout);
    assert!(!out.stderr.is_empty());
}

fn request(socket: &mut std::net::TcpStream) -> Vec<u8> {
    let mut data = Vec::new();
    let mut chunk = [0; 4096];
    loop {
        let n = socket.read(&mut chunk).unwrap();
        assert!(n > 0);
        data.extend_from_slice(&chunk[..n]);
        if complete(&data) {
            return data;
        }
    }
}
fn complete(data: &[u8]) -> bool {
    let Some(i) = data.windows(4).position(|x| x == b"\r\n\r\n") else {
        return false;
    };
    let header = String::from_utf8_lossy(&data[..i]);
    let length = header.lines().find_map(length).unwrap_or(0);
    data.len() >= i + 4 + length
}
fn length(line: &str) -> Option<usize> {
    line.to_ascii_lowercase()
        .strip_prefix("content-length:")
        .and_then(|v| v.trim().parse().ok())
}
