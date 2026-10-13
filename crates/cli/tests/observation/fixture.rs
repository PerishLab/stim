use serde_json::Value;
use std::path::PathBuf;
use std::process::{Command, Output};

pub(super) struct Fixture {
    root: tempfile::TempDir,
}

impl Fixture {
    pub(super) fn new() -> Self {
        let fixture = Self {
            root: tempfile::tempdir().unwrap(),
        };
        std::fs::create_dir(fixture.seat()).unwrap();
        fixture
    }

    pub(super) fn seat(&self) -> PathBuf {
        self.root.path().join(".stim")
    }

    pub(super) fn configure(&self, text: &str) {
        std::fs::write(self.seat().join("config.toml"), text).unwrap();
    }

    pub(super) fn run(&self, name: &str, base: &str, capability: bool) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_stim"));
        command
            .env_clear()
            .env("HOME", self.root.path())
            .env("USERPROFILE", self.root.path())
            .env("LOCALAPPDATA", self.root.path())
            .current_dir(self.root.path());
        command.args(["--base-url", base]);
        match name {
            "send" => {
                command.args([
                    "send",
                    "private-message",
                    "--as",
                    "private-person",
                    "--request-id",
                    "private-request",
                ]);
            }
            "poll" => {
                command.args(["poll", "--as", "private-person"]);
            }
            "reply" => {
                command.args(["reply", "private-reply"]);
            }
            _ => panic!("unknown fixture command"),
        }
        if capability {
            for key in [
                "SANTI_SOUL_ID",
                "SANTI_STRAND_ID",
                "SANTI_TURN_ID",
                "SANTI_TOOL_CALL_ID",
                "SANTI_EFFECT_ID",
                "SANTI_RUNTIME_CAPABILITY",
            ] {
                command.env(key, "private-capability");
            }
        }
        command.output().unwrap()
    }

    pub(super) fn records(&self) -> Vec<Value> {
        let mut atoms = Vec::new();
        for entry in std::fs::read_dir(self.seat().join("state/locus")).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy();
            if name == "active.jsonl" || name.starts_with("sealed-") {
                let text = std::fs::read_to_string(path).unwrap();
                atoms.extend(
                    text.lines()
                        .map(|line| serde_json::from_str::<Value>(line).unwrap()),
                );
            }
        }
        atoms
    }
}
