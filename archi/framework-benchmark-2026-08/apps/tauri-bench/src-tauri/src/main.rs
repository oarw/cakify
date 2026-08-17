use std::{
    env,
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    sync::Mutex,
};

use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager, State};

#[derive(Clone, Deserialize, Serialize)]
struct ReadyResponse {
    port: u16,
    protocol_version: String,
    fixture_hash: String,
    session_token: String,
    pid: u32,
}

struct CoreState {
    ready: Option<ReadyResponse>,
    child: Mutex<Option<Child>>,
}

#[tauri::command]
fn core_info(state: State<'_, CoreState>) -> Option<ReadyResponse> {
    state.ready.clone()
}

fn main() {
    let app = tauri::Builder::default()
        .setup(|app| {
            let (child, ready) = start_core(argument_value("--core-path"), argument_value("--core-ready-file"));
            app.manage(CoreState { ready: ready.clone(), child: Mutex::new(child) });
            if let Some(ready) = ready {
                let _ = app.emit("core-ready", ready);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![core_info])
        .build(tauri::generate_context!())
        .expect("error while building Tauri benchmark shell");

    app.run(|app_handle, event| {
        if matches!(event, tauri::RunEvent::Exit) {
            let state = app_handle.state::<CoreState>();
            if let Ok(mut child) = state.child.lock() {
                if let Some(child) = child.as_mut() {
                    let _ = child.kill();
                }
                child.take();
            };
        }
    });
}

fn start_core(path: Option<String>, ready_file: Option<String>) -> (Option<Child>, Option<ReadyResponse>) {
    let Some(path) = path else { return (None, None) };
    let mut command = Command::new(path);
    command.arg("--port").arg("0");
    if let Some(ready_file) = ready_file {
        command.arg("--ready-file").arg(ready_file);
    }
    command.stdout(Stdio::piped()).stderr(Stdio::null());
    let Ok(mut child) = command.spawn() else { return (None, None) };
    let ready = child
        .stdout
        .take()
        .and_then(|stdout| BufReader::new(stdout).lines().next())
        .and_then(Result::ok)
        .and_then(|line| line.strip_prefix("CAKIFY_READY ").map(str::to_owned))
        .and_then(|json| serde_json::from_str::<ReadyResponse>(&json).ok());
    match ready {
        Some(ready) => (Some(child), Some(ready)),
        None => {
            let _ = child.kill();
            (None, None)
        }
    }
}

fn argument_value(name: &str) -> Option<String> {
    let mut args = env::args().skip(1);
    while let Some(value) = args.next() {
        if value == name { return args.next(); }
    }
    None
}
