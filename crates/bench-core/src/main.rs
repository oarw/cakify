use std::{
    convert::Infallible,
    env, fs,
    io::Write,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::sse::{Event, KeepAlive, Sse},
    routing::{get, post},
    Json, Router,
};
use cakify_bench_protocol::{
    BenchEvent, CancelRequest, CancelResponse, FixtureManifest, HealthResponse, MessagePage,
    MessageRecord, ReadyResponse, PROTOCOL_VERSION,
};
use futures_util::stream::{self, Stream};
use serde::Deserialize;
use tokio::{net::TcpListener, sync::Mutex, time::sleep};
use tower_http::cors::{Any, CorsLayer};

#[derive(Clone)]
struct AppState {
    manifest: Arc<FixtureManifest>,
    cancelled_runs: Arc<Mutex<std::collections::HashSet<String>>>,
    session_token: Arc<String>,
}

#[derive(Debug, Deserialize)]
struct PageQuery {
    offset: Option<u32>,
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct EventQuery {
    run_id: Option<String>,
    scenario: Option<String>,
}

type ApiError = (StatusCode, String);

#[tokio::main]
async fn main() {
    let port = argument_u16("--port").unwrap_or(0);
    let ready_file = argument_path("--ready-file");
    let manifest = Arc::new(FixtureManifest::default());
    let session_token = Arc::new(generate_session_token());
    let state = AppState {
        manifest: manifest.clone(),
        cancelled_runs: Arc::new(Mutex::new(std::collections::HashSet::new())),
        session_token: session_token.clone(),
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/fixture/manifest", get(fixture_manifest))
        .route("/fixture/messages", get(fixture_messages))
        .route("/run/events", get(run_events))
        .route("/run/cancel", post(cancel_run))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(state);

    let listener = TcpListener::bind(("127.0.0.1", port))
        .await
        .expect("bind localhost benchmark port");
    let actual_port = listener
        .local_addr()
        .expect("read bound benchmark address")
        .port();
    let ready = ReadyResponse {
        port: actual_port,
        protocol_version: PROTOCOL_VERSION.to_owned(),
        fixture_hash: manifest.fixture_hash.clone(),
        session_token: (*session_token).clone(),
        pid: std::process::id(),
    };
    let line = serde_json::to_string(&ready).expect("serialize ready response");
    println!("CAKIFY_READY {line}");
    let _ = std::io::stdout().flush();
    if let Some(path) = ready_file {
        if let Err(error) = fs::write(path, format!("{line}\n")) {
            eprintln!("unable to write ready file: {error}");
        }
    }

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("serve benchmark core");
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

async fn health(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<HealthResponse>, ApiError> {
    authorize(&headers, &state)?;
    Ok(Json(HealthResponse {
        ok: true,
        protocol_version: state.manifest.protocol_version.clone(),
        fixture_hash: state.manifest.fixture_hash.clone(),
    }))
}

async fn fixture_manifest(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<FixtureManifest>, ApiError> {
    authorize(&headers, &state)?;
    Ok(Json((*state.manifest).clone()))
}

async fn fixture_messages(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<PageQuery>,
) -> Result<Json<MessagePage>, ApiError> {
    authorize(&headers, &state)?;
    let offset = query.offset.unwrap_or(0).min(state.manifest.message_count);
    let requested_limit = query.limit.unwrap_or(state.manifest.page_size);
    let limit = requested_limit.clamp(1, state.manifest.page_size);
    let end = offset
        .saturating_add(limit)
        .min(state.manifest.message_count);
    let messages = (offset..end).map(make_message).collect();
    Ok(Json(MessagePage {
        fixture_hash: state.manifest.fixture_hash.clone(),
        offset,
        total: state.manifest.message_count,
        messages,
    }))
}

async fn run_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<EventQuery>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    authorize(&headers, &state)?;
    let run_id = query.run_id.unwrap_or_else(|| "default-run".to_owned());
    let _scenario = query.scenario.unwrap_or_else(|| "stream".to_owned());
    let stream_state = EventState {
        run_id,
        manifest: state.manifest,
        cancelled_runs: state.cancelled_runs,
        phase: EventPhase::Ready,
        sequence: 0,
        started: Instant::now(),
    };
    let events = stream::unfold(stream_state, next_event);
    Ok(Sse::new(events).keep_alive(KeepAlive::default()))
}

async fn cancel_run(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CancelRequest>,
) -> Result<Json<CancelResponse>, ApiError> {
    authorize(&headers, &state)?;
    if request.run_id.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "run_id is required".to_owned()));
    }
    state
        .cancelled_runs
        .lock()
        .await
        .insert(request.run_id.clone());
    Ok(Json(CancelResponse {
        accepted: true,
        run_id: request.run_id,
    }))
}

fn authorize(headers: &HeaderMap, state: &AppState) -> Result<(), ApiError> {
    let supplied = headers
        .get("x-cakify-session")
        .and_then(|value| value.to_str().ok());
    if supplied == Some(state.session_token.as_str()) {
        Ok(())
    } else {
        Err((
            StatusCode::UNAUTHORIZED,
            "missing or invalid x-cakify-session header".to_owned(),
        ))
    }
}

#[derive(Clone, Copy)]
enum EventPhase {
    Ready,
    Tool,
    Stream,
    Complete,
}

struct EventState {
    run_id: String,
    manifest: Arc<FixtureManifest>,
    cancelled_runs: Arc<Mutex<std::collections::HashSet<String>>>,
    phase: EventPhase,
    sequence: u32,
    started: Instant,
}

async fn next_event(mut state: EventState) -> Option<(Result<Event, Infallible>, EventState)> {
    if matches!(state.phase, EventPhase::Complete) {
        return None;
    }
    // Every SSE stream begins with a ready marker, even if cancellation races
    // ahead of the first poll. The next poll will emit the cancelled marker.
    let cancelled = if matches!(state.phase, EventPhase::Ready) {
        false
    } else {
        state.cancelled_runs.lock().await.remove(&state.run_id)
    };
    if cancelled {
        let payload = BenchEvent::Cancelled {
            run_id: state.run_id.clone(),
            reason: "cancel_requested".to_owned(),
        };
        let event = json_event("cancelled", &payload);
        state.phase = EventPhase::Complete;
        return Some((Ok(event), state));
    }

    if matches!(state.phase, EventPhase::Tool) && state.sequence >= state.manifest.tool_event_count
    {
        state.phase = EventPhase::Stream;
        state.sequence = 0;
    }

    let (name, payload, delay) = match state.phase {
        EventPhase::Ready => {
            state.phase = EventPhase::Tool;
            (
                "ready",
                BenchEvent::Ready {
                    run_id: state.run_id.clone(),
                    fixture_hash: state.manifest.fixture_hash.clone(),
                },
                Duration::ZERO,
            )
        }
        EventPhase::Tool if state.sequence < state.manifest.tool_event_count => {
            let stages = [
                "proposed",
                "approved",
                "running",
                "output",
                "output",
                "completed",
                "failed",
                "cancelled",
            ];
            let stage = stages[state.sequence as usize % stages.len()];
            let sequence = state.sequence;
            state.sequence += 1;
            (
                "tool",
                BenchEvent::Tool {
                    run_id: state.run_id.clone(),
                    sequence,
                    tool: "fixture.search".to_owned(),
                    stage: stage.to_owned(),
                    detail: format!("deterministic tool event {sequence}"),
                },
                Duration::from_millis(80),
            )
        }
        EventPhase::Tool => unreachable!("tool phase was normalized before dispatch"),
        EventPhase::Stream if state.sequence < state.manifest.stream_event_count => {
            let sequence = state.sequence;
            let done = sequence + 1 == state.manifest.stream_event_count;
            state.sequence += 1;
            (
                "stream_delta",
                BenchEvent::StreamDelta {
                    run_id: state.run_id.clone(),
                    sequence,
                    text: format!(" fixture token {sequence:02}"),
                    done,
                },
                Duration::from_millis(state.manifest.stream_interval_ms),
            )
        }
        EventPhase::Stream => {
            state.phase = EventPhase::Complete;
            (
                "completed",
                BenchEvent::Completed {
                    run_id: state.run_id.clone(),
                    elapsed_ms: state.started.elapsed().as_millis() as u64,
                },
                Duration::ZERO,
            )
        }
        EventPhase::Complete => return None,
    };
    if !delay.is_zero() {
        sleep(delay).await;
    }
    Some((Ok(json_event(name, &payload)), state))
}

fn json_event<T: serde::Serialize>(name: &str, payload: &T) -> Event {
    Event::default()
        .event(name)
        .json_data(payload)
        .expect("serialize benchmark SSE event")
}

fn make_message(index: u32) -> MessageRecord {
    let role = match index % 4 {
        0 => "user",
        1 | 2 => "assistant",
        _ => "tool",
    };
    let markdown = match index % 7 {
        0 => format!("## Fixture message {index}\n\nA **bold** deterministic message."),
        1 => format!("- item {index}\n- stable list item"),
        2 => format!("> quoted fixture message {index}"),
        3 => format!("| key | value |\n| --- | --- |\n| index | {index} |"),
        4 => format!("```text\nfixture-{index:05}\n```"),
        5 => format!("普通中文消息 {index}，用于输入与排版一致性。"),
        _ => format!("Plain fixture message {index}."),
    };
    MessageRecord {
        id: format!("msg-{index:05}"),
        index,
        role: role.to_owned(),
        markdown,
        has_image: index == 42,
    }
}

fn generate_session_token() -> String {
    let mut bytes = [0_u8; 32];
    getrandom::getrandom(&mut bytes).expect("obtain random session token");
    hex::encode(bytes)
}

fn argument_u16(name: &str) -> Option<u16> {
    argument(name).and_then(|value| value.parse().ok())
}

fn argument_path(name: &str) -> Option<PathBuf> {
    argument(name).map(PathBuf::from)
}

fn argument(name: &str) -> Option<String> {
    let mut args = env::args().skip(1);
    while let Some(value) = args.next() {
        if value == name {
            return args.next();
        }
    }
    None
}
