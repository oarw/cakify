use cakify_core::{
    start_core, AppCommand, AppEvent, ConversationId, CoreEvents, CoreRuntime, RequestId,
};
use cakify_platform_windows::app_data_paths;
use gpui::{
    div, prelude::*, px, rgb, size, App, Bounds, Context, MouseButton, MouseUpEvent, SharedString,
    TitlebarOptions, Window, WindowBounds, WindowOptions,
};
use gpui_platform::application;

const WINDOW_WIDTH: f32 = 960.0;
const WINDOW_HEIGHT: f32 = 680.0;

struct CakifyApp {
    core: CoreRuntime,
    events: CoreEvents,
    status: SharedString,
    data_root: SharedString,
    revision: u64,
    next_request: u64,
    active_conversation: Option<ConversationId>,
}

impl CakifyApp {
    fn new(core: CoreRuntime, events: CoreEvents) -> Self {
        let data_root = app_data_paths()
            .map(|paths| paths.root.display().to_string())
            .unwrap_or_else(|_| "LOCALAPPDATA unavailable".to_owned());
        Self {
            core,
            events,
            status: "core starting".into(),
            data_root: data_root.into(),
            revision: 0,
            next_request: 1,
            active_conversation: None,
        }
    }

    fn start_event_bridge(&mut self, cx: &mut Context<Self>) {
        let events = self.events.receiver();
        cx.spawn(async move |this, cx| {
            while let Ok(event) = events.recv().await {
                if this
                    .update(cx, |app, cx| {
                        app.apply_event(event);
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
    }

    fn apply_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::CoreReady { revision } => {
                self.revision = revision;
                self.status = "core ready · M0 bridge online".into();
            }
            AppEvent::CoreStopped { revision } => {
                self.revision = revision;
                self.status = "core stopped".into();
            }
            AppEvent::ConversationCreated {
                conversation_id,
                revision,
                ..
            } => {
                self.revision = revision;
                self.active_conversation = Some(conversation_id);
                self.status = "conversation created".into();
            }
            AppEvent::DraftAccepted {
                run_id, revision, ..
            } => {
                self.revision = revision;
                self.status = format!("draft accepted · run {}", run_id.value()).into();
            }
            AppEvent::Status { message, revision } => {
                self.revision = revision;
                self.status = message.into();
            }
        }
    }

    fn new_conversation(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        let request_id = RequestId::new(self.next_request);
        self.next_request += 1;
        let result = self
            .core
            .handle()
            .try_dispatch(AppCommand::CreateConversation {
                request_id,
                title: "新会话".to_owned(),
            });
        self.status = match result {
            Ok(()) => "creating conversation…".into(),
            Err(error) => error.to_string().into(),
        };
        cx.notify();
    }

    fn send_probe(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(conversation_id) = self.active_conversation else {
            self.status = "先创建一个会话".into();
            cx.notify();
            return;
        };
        let request_id = RequestId::new(self.next_request);
        self.next_request += 1;
        let result = self.core.handle().try_dispatch(AppCommand::SubmitDraft {
            request_id,
            conversation_id,
            text: "M0 fake draft".to_owned(),
        });
        self.status = match result {
            Ok(()) => "sending fake draft…".into(),
            Err(error) => error.to_string().into(),
        };
        cx.notify();
    }
}

impl Render for CakifyApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let canvas = rgb(0xf5f7f8);
        let surface = rgb(0xffffff);
        let muted_surface = rgb(0xedf1f3);
        let text = rgb(0x172026);
        let muted_text = rgb(0x62717a);
        let border = rgb(0xd7dfe3);
        let accent = rgb(0x087f5b);
        let status = self.status.clone();
        let data_root = self.data_root.clone();
        let revision = self.revision;

        div()
            .size_full()
            .flex()
            .bg(canvas)
            .text_color(text)
            .child(
                div()
                    .w(px(252.0))
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
                            .child("轻量原生 AI Chat")
                            .child(div().mt_1().text_color(muted_text).child("GPUI + Rust")),
                    )
                    .child(div().text_sm().text_color(muted_text).child("会话"))
                    .child(div().rounded_md().bg(muted_surface).p_2().text_sm().child(
                        if self.active_conversation.is_some() {
                            "新会话"
                        } else {
                            "还没有会话"
                        },
                    ))
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted_text)
                            .child("M0 · core revision ")
                            .child(revision.to_string()),
                    )
                    .child(div().text_xs().text_color(muted_text).child(data_root)),
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
                            .child(div().text_lg().child("新会话"))
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .child(
                                        div()
                                            .id("new-conversation")
                                            .rounded_md()
                                            .border_1()
                                            .border_color(border)
                                            .px_3()
                                            .py_2()
                                            .text_sm()
                                            .cursor_pointer()
                                            .on_mouse_up(
                                                MouseButton::Left,
                                                cx.listener(Self::new_conversation),
                                            )
                                            .child("新会话"),
                                    )
                                    .child(
                                        div()
                                            .id("send-probe")
                                            .rounded_md()
                                            .bg(accent)
                                            .text_color(rgb(0xffffff))
                                            .px_3()
                                            .py_2()
                                            .text_sm()
                                            .cursor_pointer()
                                            .on_mouse_up(
                                                MouseButton::Left,
                                                cx.listener(Self::send_probe),
                                            )
                                            .child("发送测试"),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .items_center()
                            .justify_center()
                            .p_8()
                            .child(
                                div()
                                    .max_w(px(620.0))
                                    .rounded_md()
                                    .border_1()
                                    .border_color(border)
                                    .bg(surface)
                                    .p_6()
                                    .child(
                                        div()
                                            .text_color(muted_text)
                                            .child("M0 原生窗口已连接 Core command/event bridge。"),
                                    )
                                    .child(div().mt_3().text_sm().text_color(muted_text).child(
                                        "真实输入、SQLite、Provider 和密钥存储按路线图逐步接入。",
                                    )),
                            ),
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
                                    .min_h(px(92.0))
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
                                            .child("输入消息… · M2 接入 GPUI 原生 IME composer"),
                                    )
                                    .child(div().text_xs().text_color(muted_text).child(status)),
                            ),
                    ),
            )
    }
}

impl Drop for CakifyApp {
    fn drop(&mut self) {
        let _ = self.core.handle().shutdown();
    }
}

fn main() {
    let core = start_core().expect("start Cakify core");
    let events = core.events();
    application().run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(WINDOW_WIDTH), px(WINDOW_HEIGHT)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Cakify".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            move |_, cx| {
                cx.new(|cx| {
                    let mut app = CakifyApp::new(core, events);
                    app.start_event_bridge(cx);
                    app
                })
            },
        )
        .expect("open Cakify window");
        cx.activate(true);
    });
}
