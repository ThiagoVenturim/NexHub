use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::thread;

use assert_cmd::Command;
use tempfile::tempdir;

#[test]
fn test_exit_code_2_app_not_running() {
    let dir = tempdir().unwrap();
    let mut cmd = Command::cargo_bin("nexhub").unwrap();
    cmd.env("NEXHUB_HOME", dir.path())
        .env_remove("NEXHUB_PORT")
        .env_remove("NEXHUB_TOKEN")
        .arg("status")
        .assert()
        .code(2);
}

#[test]
fn test_exit_code_3_unauthorized() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            let _ = reader.read_line(&mut line);

            let resp = serde_json::json!({
                "id": "cli-req",
                "ok": false,
                "error": {
                    "code": "unauthorized",
                    "message": "invalid token"
                }
            });
            let mut resp_bytes = serde_json::to_vec(&resp).unwrap();
            resp_bytes.push(b'\n');
            let _ = stream.write_all(&resp_bytes);
        }
    });

    let mut cmd = Command::cargo_bin("nexhub").unwrap();
    cmd.env("NEXHUB_PORT", port.to_string())
        .env("NEXHUB_TOKEN", "bad_token")
        .arg("status")
        .assert()
        .code(3);
}

#[test]
fn test_exit_code_4_not_found() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            let _ = reader.read_line(&mut line);

            let resp = serde_json::json!({
                "id": "cli-req",
                "ok": false,
                "error": {
                    "code": "not_found",
                    "message": "agent missing not found"
                }
            });
            let mut resp_bytes = serde_json::to_vec(&resp).unwrap();
            resp_bytes.push(b'\n');
            let _ = stream.write_all(&resp_bytes);
        }
    });

    let mut cmd = Command::cargo_bin("nexhub").unwrap();
    cmd.env("NEXHUB_PORT", port.to_string())
        .env("NEXHUB_TOKEN", "token")
        .args(["ask", "missing", "hello"])
        .assert()
        .code(4);
}

#[test]
fn test_exit_code_5_timeout() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            let _ = reader.read_line(&mut line);

            let resp = serde_json::json!({
                "id": "cli-req",
                "ok": false,
                "error": {
                    "code": "timeout",
                    "message": "ask timed out after 30s"
                }
            });
            let mut resp_bytes = serde_json::to_vec(&resp).unwrap();
            resp_bytes.push(b'\n');
            let _ = stream.write_all(&resp_bytes);
        }
    });

    let mut cmd = Command::cargo_bin("nexhub").unwrap();
    cmd.env("NEXHUB_PORT", port.to_string())
        .env("NEXHUB_TOKEN", "token")
        .args(["ask", "slow-agent", "hello", "--timeout", "10s"])
        .assert()
        .code(5);
}

#[test]
fn test_file_and_stdin_args() {
    let dir = tempdir().unwrap();
    let test_file = dir.path().join("brief.txt");
    std::fs::write(&test_file, "this is a long brief").unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            let _ = reader.read_line(&mut line);

            let req: serde_json::Value = serde_json::from_str(&line).unwrap();
            assert_eq!(
                req["params"]["body"].as_str().unwrap(),
                "this is a long brief"
            );

            let resp = serde_json::json!({
                "id": "cli-req",
                "ok": true,
                "result": { "message_id": "msg-file-1" }
            });
            let mut resp_bytes = serde_json::to_vec(&resp).unwrap();
            resp_bytes.push(b'\n');
            let _ = stream.write_all(&resp_bytes);
        }
    });

    let mut cmd = Command::cargo_bin("nexhub").unwrap();
    let file_arg = format!("@{}", test_file.display());
    cmd.env("NEXHUB_PORT", port.to_string())
        .env("NEXHUB_TOKEN", "token")
        .args(["notify", "worker", &file_arg])
        .assert()
        .success()
        .stdout(predicates::str::contains("msg-file-1"));
}
