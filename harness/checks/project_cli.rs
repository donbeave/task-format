//! First-class project/group commands against real files and loopback HTTP boundaries.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::Duration;

use clap::Parser;
use serde_json::{Value, json};
use taskfmt::cli::{Cli, Command as CliCommand, GroupCmd, ProjectCmd};

const EXECUTION: &str = "00000000-0000-4000-8000-000000000001";
const OTHER: &str = "00000000-0000-4000-8000-000000000002";

fn invoke(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_taskfmt"))
        .args(args)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn document(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "JSON output: {error}; stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn fixture() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    let projects = root.path().join("projects");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/example");
    for (group, status) in [("build", "pending"), ("docs", "draft")] {
        let task = projects.join(format!("alpha/{group}/001"));
        std::fs::create_dir_all(&task).unwrap();
        std::fs::write(
            projects.join("alpha/README.md"),
            "# Alpha\n\nProject description.\n",
        )
        .unwrap();
        std::fs::write(
            projects.join(format!("alpha/{group}/README.md")),
            format!("# {group}\n\nGroup description.\n"),
        )
        .unwrap();
        for file in ["README.md", "verify.toml"] {
            std::fs::copy(source.join(file), task.join(file)).unwrap();
        }
        std::fs::write(
            task.join("task.toml"),
            format!("schema = \"task-meta/v1\"\nstatus = \"{status}\"\ndependencies = []\n"),
        )
        .unwrap();
    }
    std::fs::create_dir_all(projects.join("beta")).unwrap();
    std::fs::write(projects.join("beta/README.md"), "# Beta\n").unwrap();
    root
}

#[test]
fn project_and_group_reads_use_real_catalog_and_mark_metadata_authority() {
    let root = fixture();
    let path = root.path().join("projects");
    let path = path.to_str().unwrap();
    let output = invoke(&["project", "list", "--projects-root", path, "--json"]);
    assert!(output.status.success());
    let value = document(&output);
    assert_eq!(value["schema"], "taskfmt/project-cli/v1");
    assert_eq!(value["data"]["authority"], "filesystem_metadata");
    assert_eq!(value["data"]["items"][0]["project"]["code"], "alpha");
    assert_eq!(value["data"]["items"][0]["counts"]["pending"], 1);
    assert_eq!(value["data"]["items"][0]["counts"]["draft"], 1);
    assert!(
        value["data"]["items"][0]["counts"]
            .get("progress")
            .is_none()
    );
    let output = invoke(&[
        "project",
        "show",
        "alpha",
        "--projects-root",
        path,
        "--json",
    ]);
    assert!(output.status.success());
    assert_eq!(
        document(&output)["data"]["groups"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let output = invoke(&["group", "list", "alpha", "--projects-root", path, "--json"]);
    assert!(output.status.success());
    assert_eq!(
        document(&output)["data"]["items"].as_array().unwrap().len(),
        2
    );
    let output = invoke(&[
        "group",
        "show",
        "alpha/build",
        "--projects-root",
        path,
        "--json",
    ]);
    assert!(output.status.success());
    assert_eq!(
        document(&output)["data"]["tasks"][0]["metadata"]["status"],
        "pending"
    );
    let output = invoke(&["group", "show", "alpha/build", "--projects-root", path]);
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .contains("live run progress and verification evidence are not loaded")
    );
}

#[test]
fn lint_checks_scoped_packages_and_invalid_catalog_fails_as_typed_json() {
    let root = fixture();
    let path = root.path().join("projects");
    for (kind, id, count) in [("project", "alpha", 2), ("group", "alpha/build", 1)] {
        let output = invoke(&[
            kind,
            "lint",
            id,
            "--projects-root",
            path.to_str().unwrap(),
            "--json",
        ]);
        assert!(output.status.success());
        let value = document(&output);
        assert_eq!(value["data"]["passed"], true);
        assert_eq!(value["data"]["reports"].as_array().unwrap().len(), count);
    }
    std::fs::write(
        path.join("alpha/build/001/task.toml"),
        "schema = \"task-meta/v1\"\nstatus = \"pending\"\ndependencies = [\"alpha/build/999\"]\n",
    )
    .unwrap();
    let output = invoke(&[
        "project",
        "lint",
        "alpha",
        "--projects-root",
        path.to_str().unwrap(),
        "--json",
    ]);
    assert!(!output.status.success());
    assert_eq!(document(&output)["error"]["code"], "catalog_invalid");
}

#[test]
fn defaults_resolve_projects_relative_to_explicit_manifest_and_legacy_parser_stays_valid() {
    let root = fixture();
    let config = root.path().join("experiment.toml");
    std::fs::write(&config,"schema = \"experiment/v1\"\n[paths]\nprojects_dir = \"projects\"\n[agents.profiles.test]\nkind = \"codex\"\nmodel = \"test\"\neffort = \"low\"\nimage = \"test\"\n[agents.default]\nprofile = \"test\"\n").unwrap();
    let output = invoke(&[
        "--config",
        config.to_str().unwrap(),
        "project",
        "list",
        "--json",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(
        document(&output)["data"]["items"].as_array().unwrap().len(),
        2
    );
    let parsed = Cli::try_parse_from([
        "taskfmt",
        "run",
        "--task",
        "TASK-001",
        "--repo",
        "/tmp/repo.git",
        "--wait",
        "--auto",
    ])
    .unwrap();
    assert!(matches!(parsed.command,CliCommand::Run { task,wait:true,.. } if task=="TASK-001"));
    assert!(matches!(
        Cli::try_parse_from(["taskfmt", "project", "show", "alpha"])
            .unwrap()
            .command,
        CliCommand::Project {
            cmd: ProjectCmd::Show { .. }
        }
    ));
    assert!(matches!(
        Cli::try_parse_from(["taskfmt", "group", "run", "alpha/build", "--auto"])
            .unwrap()
            .command,
        CliCommand::Group {
            cmd: GroupCmd::Run { .. }
        }
    ));
}

#[test]
fn invalid_and_missing_identifiers_are_distinct_typed_errors() {
    let root = fixture();
    let path = root.path().join("projects");
    for (id, code) in [
        ("alpha/../build", "invalid_identifier"),
        ("alpha/missing", "not_found"),
    ] {
        let output = invoke(&[
            "group",
            "show",
            id,
            "--projects-root",
            path.to_str().unwrap(),
            "--json",
        ]);
        assert!(!output.status.success());
        assert_eq!(document(&output)["error"]["code"], code);
    }
}

fn execution(id: &str, state: &str) -> Value {
    json!({"id":id,"task_ids":["alpha/build/001"],"state":state,"started":"2026-09-07T00:00:00Z","finished":if state=="running" {None} else {Some("2026-09-07T00:00:01Z")},"tasks":{}})
}

struct Reply {
    method: &'static str,
    path: &'static str,
    status: u16,
    body: Value,
}

fn server(replies: Vec<Reply>) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let worker = thread::spawn(move || {
        listener.set_nonblocking(true).unwrap();
        for reply in replies {
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            && std::time::Instant::now() < deadline =>
                    {
                        thread::sleep(Duration::from_millis(10))
                    }
                    Err(error) => panic!("HTTP fixture accept: {error}"),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut request = Vec::new();
            let mut byte = [0];
            while !request.ends_with(b"\r\n\r\n") {
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
                assert!(request.len() < 8192);
            }
            let request = String::from_utf8(request).unwrap();
            assert!(
                request.starts_with(&format!("{} {} HTTP/1.1\r\n", reply.method, reply.path)),
                "{request}"
            );
            if reply.method == "POST" {
                assert!(
                    request
                        .to_ascii_lowercase()
                        .contains("x-task-monitor: 1\r\n")
                );
            }
            let body = reply.body.to_string();
            let location = if reply.status == 302 {
                format!("Location: {}\r\n", reply.body["location"].as_str().unwrap())
            } else {
                String::new()
            };
            write!(stream,"HTTP/1.1 {} Response\r\n{}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",reply.status,location,body.len(),body).unwrap();
        }
    });
    (address, worker)
}

#[test]
fn run_wait_tracks_only_returned_execution_through_live_http() {
    let (address, worker) = server(vec![
        Reply {
            method: "POST",
            path: "/api/run/project/alpha",
            status: 202,
            body: execution(EXECUTION, "running"),
        },
        Reply {
            method: "GET",
            path: "/api/executions",
            status: 200,
            body: json!([execution(OTHER, "done"), execution(EXECUTION, "running")]),
        },
        Reply {
            method: "GET",
            path: "/api/executions",
            status: 200,
            body: json!([execution(OTHER, "failed"), execution(EXECUTION, "done")]),
        },
    ]);
    let output = invoke(&[
        "project",
        "run",
        "alpha",
        "--monitor-url",
        &address,
        "--wait",
        "--json",
        "--auto",
    ]);
    worker.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(document(&output)["data"]["execution"]["id"], EXECUTION);
    assert_eq!(document(&output)["data"]["execution"]["state"], "done");
}

#[test]
fn group_run_preserves_server_rejection_and_terminal_failure_exit() {
    let (address, worker) = server(vec![Reply {
        method: "POST",
        path: "/api/run/group/alpha/build",
        status: 409,
        body: json!({"error":{"code":"not_eligible","message":"Dependency unfinished"}}),
    }]);
    let output = invoke(&[
        "group",
        "run",
        "alpha/build",
        "--monitor-url",
        &address,
        "--json",
        "--yes",
    ]);
    worker.join().unwrap();
    assert!(!output.status.success());
    let value = document(&output);
    assert_eq!(value["error"]["http_status"], 409);
    assert_eq!(value["error"]["monitor_code"], "not_eligible");
    let (address, worker) = server(vec![
        Reply {
            method: "POST",
            path: "/api/run/group/alpha/build",
            status: 202,
            body: execution(EXECUTION, "running"),
        },
        Reply {
            method: "GET",
            path: "/api/executions",
            status: 200,
            body: json!([execution(EXECUTION, "failed")]),
        },
    ]);
    let output = invoke(&[
        "group",
        "run",
        "alpha/build",
        "--monitor-url",
        &address,
        "--wait",
        "--json",
        "--auto",
    ]);
    worker.join().unwrap();
    assert!(!output.status.success());
    assert_eq!(document(&output)["data"]["execution"]["state"], "failed");
}

#[test]
fn run_requires_consent_before_any_http_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let output = invoke(&[
        "project",
        "run",
        "alpha",
        "--monitor-url",
        &address,
        "--json",
    ]);
    assert!(!output.status.success());
    assert_eq!(document(&output)["error"]["code"], "consent_required");
    listener.set_nonblocking(true).unwrap();
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[test]
fn remote_credentialed_and_nonorigin_urls_are_rejected() {
    for address in [
        "https://127.0.0.1:3001",
        "http://example.com:3001",
        "http://127.0.0.1:3001/api",
        "http://user@localhost:3001",
        "http://localhost:3001/?query=1",
        "http://localhost:3001/#fragment",
    ] {
        let output = invoke(&[
            "project",
            "run",
            "alpha",
            "--monitor-url",
            address,
            "--auto",
            "--json",
        ]);
        assert!(!output.status.success(), "{address}");
        assert_eq!(
            document(&output)["error"]["code"],
            "invalid_monitor_url",
            "{address}"
        );
    }
}

#[test]
fn malformed_success_response_is_not_execution_success() {
    let (address, worker) = server(vec![Reply {
        method: "POST",
        path: "/api/run/project/alpha",
        status: 202,
        body: json!({"state":"done"}),
    }]);
    let output = invoke(&[
        "project",
        "run",
        "alpha",
        "--monitor-url",
        &address,
        "--auto",
        "--json",
    ]);
    worker.join().unwrap();
    assert!(!output.status.success());
    assert_eq!(document(&output)["error"]["code"], "invalid_response");
}

#[test]
fn redirects_never_connect_to_the_location_target() {
    let trap = TcpListener::bind("127.0.0.1:0").unwrap();
    let target = format!("http://{}/", trap.local_addr().unwrap());
    let (address, worker) = server(vec![Reply {
        method: "POST",
        path: "/api/run/project/alpha",
        status: 302,
        body: json!({"location":target}),
    }]);
    let output = invoke(&[
        "project",
        "run",
        "alpha",
        "--monitor-url",
        &address,
        "--auto",
        "--json",
    ]);
    worker.join().unwrap();
    assert!(!output.status.success());
    assert_eq!(document(&output)["error"]["http_status"], 302);
    trap.set_nonblocking(true).unwrap();
    assert_eq!(
        trap.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[test]
fn proxy_environment_never_receives_a_monitor_request() {
    let trap = TcpListener::bind("127.0.0.1:0").unwrap();
    let proxy = format!("http://{}", trap.local_addr().unwrap());
    let (address, worker) = server(vec![Reply {
        method: "POST",
        path: "/api/run/project/alpha",
        status: 202,
        body: execution(EXECUTION, "running"),
    }]);
    let output = Command::new(env!("CARGO_BIN_EXE_taskfmt"))
        .args([
            "project",
            "run",
            "alpha",
            "--monitor-url",
            &address,
            "--auto",
            "--json",
        ])
        .env("HTTP_PROXY", &proxy)
        .env("http_proxy", &proxy)
        .env("ALL_PROXY", &proxy)
        .env("all_proxy", &proxy)
        .env("NO_PROXY", "")
        .env("no_proxy", "")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    worker.join().unwrap();
    assert!(output.status.success());
    assert_eq!(document(&output)["data"]["execution"]["state"], "running");
    trap.set_nonblocking(true).unwrap();
    assert_eq!(
        trap.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[test]
fn wait_rejects_missing_execution_and_does_not_confuse_other_terminal_states_with_done() {
    for state in ["interrupted", "cancelled", "unrecognized", "missing"] {
        let journal = if state == "missing" {
            execution(OTHER, "done")
        } else {
            execution(EXECUTION, state)
        };
        let (address, worker) = server(vec![
            Reply {
                method: "POST",
                path: "/api/run/project/alpha",
                status: 202,
                body: execution(EXECUTION, "running"),
            },
            Reply {
                method: "GET",
                path: "/api/executions",
                status: 200,
                body: json!([journal]),
            },
        ]);
        let output = invoke(&[
            "project",
            "run",
            "alpha",
            "--monitor-url",
            &address,
            "--wait",
            "--auto",
            "--json",
        ]);
        worker.join().unwrap();
        assert!(!output.status.success(), "{state}");
        let value = document(&output);
        match state {
            "missing" => assert_eq!(value["error"]["code"], "execution_missing"),
            "unrecognized" => assert_eq!(value["error"]["code"], "invalid_response"),
            _ => assert_eq!(value["data"]["execution"]["state"], state),
        }
    }
}
