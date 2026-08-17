use std::{
    env,
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::Duration,
};

use cakify_bench_protocol::{MessagePage, MessageRecord, ReadyResponse};
use gpui::{
    App, Bounds, Context, MouseButton, MouseUpEvent, SharedString, Window, WindowBounds,
    WindowOptions, div, prelude::*, px, rgb, size, uniform_list,
};
use gpui_platform::application;

const WINDOW_WIDTH: f32 = 1280.0;
const WINDOW_HEIGHT: f32 = 800.0;

struct CoreConnection {
    child: Option<Child>,
    ready: Option<ReadyResponse>,
}

struct BenchApp {
    core: CoreConnection,
    messages: Vec<MessageRecord>,
    total_messages: usize,
    status: SharedString,
    dark: bool,
}

impl BenchApp {
    fn toggle_theme(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.dark = !self.dark;
        cx.notify();
    }

    fn run_fixture(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.status = if self.core.ready.is_some() {
            "core 已就绪 · 工具时间线可取消".into()
        } else {
            "等待 core · CI 会传入 --core-path".into()
        };
        cx.notify();
    }
}

impl Drop for BenchApp {
    fn drop(&mut self) {
        if let Some(mut child) = self.core.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Render for BenchApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (canvas, surface, muted_surface, text, muted_text, border, accent) = if self.dark {
            (
                rgb(0x15191d),
                rgb(0x20262b),
                rgb(0x2a3238),
                rgb(0xe8edf0),
                rgb(0xa7b2b9),
                rgb(0x3a454d),
                rgb(0x66c2c8),
            )
        } else {
            (
                rgb(0xf7f8fa),
                rgb(0xffffff),
                rgb(0xeef1f4),
                rgb(0x182026),
                rgb(0x64717b),
                rgb(0xd7dee4),
                rgb(0x1f6f78),
            )
        };
        let status = self.status.clone();
        let total = self.total_messages;
        let messages = self.messages.clone();

        div()
            .size_full()
            .flex()
            .bg(canvas)
            .text_color(text)
            .child(
                div()
                    .w(px(264.0))
                    .h_full()
                    .flex()
                    .flex_col()
                    .bg(surface)
                    .border_r_1()
                    .border_color(border)
                    .p_4()
                    .gap_3()
                    .child(div().text_xl().child("Cakify"))
                    .child(
                        div()
                            .rounded_md()
                            .bg(muted_surface)
                            .p_3()
                            .text_sm()
                            .child("Benchmark workspace")
                            .child(div().mt_1().text_color(muted_text).child("GPUI + Rust")),
                    )
                    .child(div().text_sm().text_color(muted_text).child("会话"))
                    .children((0..6).map(|index| {
                        div()
                            .id(index)
                            .rounded_md()
                            .p_2()
                            .bg(if index == 0 { muted_surface } else { surface })
                            .child(if index == 0 {
                                "10k fixture / active"
                            } else {
                                "Archived conversation"
                            })
                    }))
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted_text)
                            .child("同一 Rust core · deterministic fixture"),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .h(px(64.0))
                            .flex()
                            .items_center()
                            .justify_between()
                            .px_6()
                            .bg(surface)
                            .border_b_1()
                            .border_color(border)
                            .child(div().text_lg().child("New conversation"))
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .child(
                                        div()
                                            .id("theme-toggle")
                                            .rounded_md()
                                            .border_1()
                                            .border_color(border)
                                            .px_3()
                                            .py_2()
                                            .text_sm()
                                            .cursor_pointer()
                                            .on_mouse_up(
                                                MouseButton::Left,
                                                cx.listener(Self::toggle_theme),
                                            )
                                            .child(if self.dark { "亮色" } else { "暗色" }),
                                    )
                                    .child(
                                        div()
                                            .rounded_md()
                                            .bg(accent)
                                            .text_color(rgb(0xffffff))
                                            .px_3()
                                            .py_2()
                                            .text_sm()
                                            .cursor_pointer()
                                            .on_mouse_up(
                                                MouseButton::Left,
                                                cx.listener(Self::run_fixture),
                                            )
                                            .child("运行 fixture"),
                                    ),
                            ),
                    )
                    .child(
                        uniform_list(
                            "messages",
                            total,
                            cx.processor(move |_this, range, _window, _cx| {
                                range
                                    .map(|index| {
                                        let message = messages.get(index);
                                        let label = message
                                            .map(format_message)
                                            .unwrap_or_else(|| format!("加载 fixture 消息 {index:05}"));
                                        div()
                                            .id(index)
                                            .w_full()
                                            .px_6()
                                            .py_3()
                                            .child(
                                                div()
                                                    .max_w(px(760.0))
                                                    .rounded_md()
                                                    .bg(if index % 4 == 0 { surface } else { muted_surface })
                                                    .border_1()
                                                    .border_color(border)
                                                    .p_3()
                                                    .child(label),
                                            )
                                    })
                                    .collect::<Vec<_>>()
                            }),
                        )
                        .flex_1(),
                    )
                    .child(
                        div()
                            .px_6()
                            .py_4()
                            .bg(surface)
                            .border_t_1()
                            .border_color(border)
                            .child(
                                div()
                                    .min_h(px(112.0))
                                    .rounded_md()
                                    .border_1()
                                    .border_color(border)
                                    .p_3()
                                    .flex()
                                    .flex_col()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_color(muted_text)
                                            .child("输入消息… 支持中文 IME（GPUI 原生输入将在下一轮接入）"),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .justify_between()
                                            .items_center()
                                            .child(div().text_xs().text_color(muted_text).child(status))
                                            .child(
                                                div()
                                                    .rounded_md()
                                                    .bg(accent)
                                                    .text_color(rgb(0xffffff))
                                                    .px_3()
                                                    .py_2()
                                                    .text_sm()
                                                    .child("发送"),
                                            ),
                                    ),
                            ),
                    ),
            )
    }
}

fn format_message(message: &MessageRecord) -> String {
    format!("{}  ·  {}", message.role, message.markdown.replace('\n', "  "))
}

fn main() {
    let (core, messages, status) = bootstrap();
    application().run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(WINDOW_WIDTH), px(WINDOW_HEIGHT)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            move |_, cx| {
                cx.new(|_| BenchApp {
                    core,
                    messages,
                    total_messages: 10_000,
                    status: status.clone().into(),
                    dark: false,
                })
            },
        )
        .expect("open GPUI benchmark window");
        cx.activate(true);
    });
}

fn bootstrap() -> (CoreConnection, Vec<MessageRecord>, String) {
    let Some(core_path) = argument_path("--core-path") else {
        return (
            CoreConnection {
                child: None,
                ready: None,
            },
            Vec::new(),
            "未传入 core 路径 · 静态 UI 模式".to_owned(),
        );
    };

    let ready_file = argument_path("--core-ready-file");
    let mut command = Command::new(core_path);
    command.arg("--port").arg("0");
    if let Some(path) = ready_file {
        command.arg("--ready-file").arg(path);
    }
    command.stdout(Stdio::piped()).stderr(Stdio::null());
    let Ok(mut child) = command.spawn() else {
        return (
            CoreConnection {
                child: None,
                ready: None,
            },
            Vec::new(),
            "core 启动失败".to_owned(),
        );
    };

    let ready = child
        .stdout
        .take()
        .and_then(|stdout| BufReader::new(stdout).lines().next())
        .and_then(Result::ok)
        .and_then(|line| line.strip_prefix("CAKIFY_READY ").map(str::to_owned))
        .and_then(|json| serde_json::from_str::<ReadyResponse>(&json).ok());

    let Some(ready) = ready else {
        let _ = child.kill();
        return (
            CoreConnection {
                child: None,
                ready: None,
            },
            Vec::new(),
            "core ready 解析失败".to_owned(),
        );
    };

    let messages = fetch_page(&ready, 0, 200).unwrap_or_default();
    (
        CoreConnection {
            child: Some(child),
            ready: Some(ready),
        },
        messages,
        "core ready · 已载入首个分页".to_owned(),
    )
}

fn fetch_page(ready: &ReadyResponse, offset: u32, limit: u32) -> Option<Vec<MessageRecord>> {
    let body = http_get(
        ready,
        &format!("/fixture/messages?offset={offset}&limit={limit}"),
    )?;
    serde_json::from_str::<MessagePage>(&body)
        .ok()
        .map(|page| page.messages)
}

fn http_get(ready: &ReadyResponse, path: &str) -> Option<String> {
    let mut stream = TcpStream::connect(("127.0.0.1", ready.port)).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(3))).ok()?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nx-cakify-session: {}\r\nConnection: close\r\n\r\n",
        ready.session_token
    )
    .ok()?;
    let mut response = String::new();
    stream.read_to_string(&mut response).ok()?;
    response.split_once("\r\n\r\n").map(|(_, body)| body.to_owned())
}

fn argument_path(name: &str) -> Option<PathBuf> {
    let mut args = env::args().skip(1);
    while let Some(value) = args.next() {
        if value == name {
            return args.next().map(PathBuf::from);
        }
    }
    None
}
