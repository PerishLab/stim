use std::io::Write as _;

use stim::config::Config;

#[test]
fn rotation() {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    file.write_all(
        br#"
[store]
kind = "file"
path = "stim.sqlite"

[santi]
url = "http://127.0.0.1:43307"
credential = "STIM_SANTI_TOKEN"
soul = "soul_default"

[reply]
address = "127.0.0.1:43309"
issuer = "santi.example"
audience = "stim.reply"
maximum_ttl_seconds = 300

[reply_keys]
active = "6kpsY-KcUgq-9VB7Ey7F-ZVHdq6-vnuSQh7qaRRG0iw"
retiring = "6kpsY-KcUgq-9VB7Ey7F-ZVHdq6-vnuSQh7qaRRG0iw"
"#,
    )
    .unwrap();
    let config = Config::load(file.path()).unwrap();
    assert_eq!(config.reply_keys.len(), 2);
    assert!(config.store.path.ends_with("stim.sqlite"));
}
