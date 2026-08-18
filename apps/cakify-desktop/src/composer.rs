//! Minimal native text editing for Cakify.
//!
//! The shaping and IME boundary is adapted from GPUI's Apache-2.0
//! `view_example/example_editor.rs` at the workspace-pinned Zed revision.

use std::{ops::Range, time::Duration};

use gpui::{
    actions, fill, hsla, point, prelude::*, px, relative, size, App, Bounds, Context,
    CursorStyle, ElementInputHandler, Entity, EntityInputHandler, FocusHandle, Focusable,
    InteractiveElement, KeyBinding, LayoutId, PaintQuad, Pixels, ShapedLine, SharedString,
    Subscription, Task, TextRun, UTF16Selection, Window,
};
use unicode_segmentation::UnicodeSegmentation;

actions!(
    cakify_input,
    [Backspace, Delete, Left, Right, Home, End, Newline, Submit]
);

pub fn bind_input_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, Some("TextInput")),
        KeyBinding::new("delete", Delete, Some("TextInput")),
        KeyBinding::new("left", Left, Some("TextInput")),
        KeyBinding::new("right", Right, Some("TextInput")),
        KeyBinding::new("home", Home, Some("TextInput")),
        KeyBinding::new("end", End, Some("TextInput")),
        KeyBinding::new("shift-enter", Newline, Some("TextInput")),
        KeyBinding::new("enter", Submit, Some("TextInput")),
    ]);
}

pub struct TextEditor {
    value: Entity<String>,
    pub focus_handle: FocusHandle,
    cursor: usize,
    cursor_visible: bool,
    placeholder: SharedString,
    masked: bool,
    multiline: bool,
    _blink_task: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl TextEditor {
    pub fn new(
        text: impl Into<String>,
        placeholder: impl Into<SharedString>,
        masked: bool,
        multiline: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let value = cx.new(|_| text.into());
        let cursor = value.read(cx).len();
        let focus_handle = cx.focus_handle();
        let focus_subscription = cx.on_focus(&focus_handle, window, |this, _window, cx| {
            this.start_blink(cx);
        });
        let blur_subscription = cx.on_blur(&focus_handle, window, |this, _window, cx| {
            this.stop_blink(cx);
        });
        let value_subscription = cx.observe(&value, |this, value, cx| {
            let content = value.read(cx);
            let mut cursor = this.cursor.min(content.len());
            while cursor > 0 && !content.is_char_boundary(cursor) {
                cursor -= 1;
            }
            this.cursor = cursor;
            cx.notify();
        });

        Self {
            value,
            focus_handle,
            cursor,
            cursor_visible: false,
            placeholder: placeholder.into(),
            masked,
            multiline,
            _blink_task: Task::ready(()),
            _subscriptions: vec![
                focus_subscription,
                blur_subscription,
                value_subscription,
            ],
        }
    }

    pub fn text(&self, cx: &App) -> String {
        self.value.read(cx).clone()
    }

    pub fn set_text(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        let text = text.into();
        self.cursor = text.len();
        self.value.update(cx, |value, cx| {
            *value = text;
            cx.notify();
        });
        cx.notify();
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.set_text(String::new(), cx);
    }

    fn start_blink(&mut self, cx: &mut Context<Self>) {
        self.cursor_visible = true;
        self._blink_task = Self::spawn_blink_task(cx);
    }

    fn stop_blink(&mut self, cx: &mut Context<Self>) {
        self.cursor_visible = false;
        self._blink_task = Task::ready(());
        cx.notify();
    }

    fn spawn_blink_task(cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(500))
                .await;
            if this
                .update(cx, |editor, cx| {
                    editor.cursor_visible = !editor.cursor_visible;
                    cx.notify();
                })
                .is_err()
            {
                break;
            }
        })
    }

    fn reset_blink(&mut self, cx: &mut Context<Self>) {
        self.cursor_visible = true;
        self._blink_task = Self::spawn_blink_task(cx);
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        let content = self.text(cx);
        if self.cursor > 0 {
            self.cursor = previous_boundary(&content, self.cursor);
        }
        self.reset_blink(cx);
        cx.notify();
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        let content = self.text(cx);
        if self.cursor < content.len() {
            self.cursor = next_boundary(&content, self.cursor);
        }
        self.reset_blink(cx);
        cx.notify();
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        let content = self.text(cx);
        self.cursor = line_start(&content, self.cursor);
        self.reset_blink(cx);
        cx.notify();
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        let content = self.text(cx);
        self.cursor = line_end(&content, self.cursor);
        self.reset_blink(cx);
        cx.notify();
    }

    fn backspace(&mut self, _: &Backspace, _: &mut Window, cx: &mut Context<Self>) {
        if self.cursor == 0 {
            return;
        }
        let content = self.text(cx);
        let previous = previous_boundary(&content, self.cursor);
        let cursor = self.cursor;
        self.value.update(cx, |value, cx| {
            value.drain(previous..cursor);
            cx.notify();
        });
        self.cursor = previous;
        self.reset_blink(cx);
        cx.notify();
    }

    fn delete(&mut self, _: &Delete, _: &mut Window, cx: &mut Context<Self>) {
        let content = self.text(cx);
        if self.cursor >= content.len() {
            return;
        }
        let next = next_boundary(&content, self.cursor);
        let cursor = self.cursor;
        self.value.update(cx, |value, cx| {
            value.drain(cursor..next);
            cx.notify();
        });
        self.reset_blink(cx);
        cx.notify();
    }

    fn newline(&mut self, _: &Newline, _: &mut Window, cx: &mut Context<Self>) {
        if !self.multiline {
            return;
        }
        let cursor = self.cursor;
        self.value.update(cx, |value, cx| {
            value.insert(cursor, '\n');
            cx.notify();
        });
        self.cursor += 1;
        self.reset_blink(cx);
        cx.notify();
    }
}

impl Focusable for TextEditor {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EntityInputHandler for TextEditor {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<String> {
        let content = self.text(cx);
        let range = range_from_utf16(&content, &range_utf16);
        actual_range.replace(range_to_utf16(&content, &range));
        Some(content[range].to_owned())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let content = self.text(cx);
        let cursor = offset_to_utf16(&content, self.cursor);
        Some(UTF16Selection {
            range: cursor..cursor,
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        None
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {}

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let new_text = if self.multiline {
            new_text.to_owned()
        } else {
            new_text.replace(['\r', '\n'], "")
        };
        let content = self.text(cx);
        let range = range_utf16
            .as_ref()
            .map(|range| range_from_utf16(&content, range))
            .unwrap_or(self.cursor..self.cursor);
        let replacement = content[..range.start].to_owned() + &new_text + &content[range.end..];
        self.cursor = range.start + new_text.len();
        self.value.update(cx, |value, cx| {
            *value = replacement;
            cx.notify();
        });
        self.reset_blink(cx);
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range_utf16: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.replace_text_in_range(range_utf16, new_text, window, cx);
    }

    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        _bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        None
    }

    fn character_index_for_point(
        &mut self,
        _point: gpui::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}

impl Render for TextEditor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        EditorText {
            editor: cx.entity(),
        }
    }
}

struct EditorText {
    editor: Entity<TextEditor>,
}

struct EditorTextPrepaint {
    lines: Vec<ShapedLine>,
    cursor: Option<PaintQuad>,
}

impl IntoElement for EditorText {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for EditorText {
    type RequestLayoutState = ();
    type PrepaintState = EditorTextPrepaint;

    fn id(&self) -> Option<gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&gpui::GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let editor = self.editor.read(cx);
        let content = editor.value.read(cx);
        let line_count = content.split('\n').count().max(1);
        let mut style = gpui::Style::default();
        style.size.width = relative(1.).into();
        style.size.height = (window.line_height() * line_count as f32).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&gpui::GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let editor = self.editor.read(cx);
        let content = editor.value.read(cx).clone();
        let display_content = if editor.masked && !content.is_empty() {
            content
                .chars()
                .map(|character| if character == '\n' { '\n' } else { '*' })
                .collect::<String>()
        } else {
            content.clone()
        };
        let cursor_offset = if editor.masked {
            content[..editor.cursor].chars().count()
        } else {
            editor.cursor
        };
        let is_focused = editor.focus_handle.is_focused(window);
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line_height = window.line_height();
        let is_placeholder = display_content.is_empty();

        let lines = if is_placeholder {
            let placeholder = editor.placeholder.clone();
            let run = TextRun {
                len: placeholder.len(),
                font: style.font(),
                color: hsla(198. / 360., 0.08, 0.45, 0.7),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            vec![window
                .text_system()
                .shape_line(placeholder, font_size, &[run], None)]
        } else {
            display_content
                .split('\n')
                .map(|line| {
                    let text: SharedString = line.to_owned().into();
                    let run = TextRun {
                        len: text.len(),
                        font: style.font(),
                        color: style.color,
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    };
                    window
                        .text_system()
                        .shape_line(text, font_size, &[run], None)
                })
                .collect::<Vec<_>>()
        };

        let cursor = if is_focused && editor.cursor_visible {
            let (cursor_line, offset_in_line) =
                cursor_line_and_offset(&display_content, cursor_offset);
            let cursor_line = cursor_line.min(lines.len().saturating_sub(1));
            let cursor_x = lines[cursor_line].x_for_index(offset_in_line);
            Some(fill(
                Bounds::new(
                    point(
                        bounds.left() + cursor_x,
                        bounds.top() + line_height * cursor_line as f32,
                    ),
                    size(px(1.5), line_height),
                ),
                style.color,
            ))
        } else {
            None
        };

        EditorTextPrepaint { lines, cursor }
    }

    fn paint(
        &mut self,
        _id: Option<&gpui::GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.editor.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.editor.clone()),
            cx,
        );
        let line_height = window.line_height();
        for (index, line) in prepaint.lines.iter().enumerate() {
            let origin = point(
                bounds.left(),
                bounds.top() + line_height * index as f32,
            );
            line.paint(origin, line_height, gpui::TextAlign::Left, None, window, cx)
                .expect("paint editor text");
        }
        if let Some(cursor) = prepaint.cursor.take() {
            window.paint_quad(cursor);
        }
    }
}

pub fn editor_actions<E: InteractiveElement>(editor: Entity<TextEditor>) -> impl FnOnce(E) -> E {
    move |element| {
        element
            .on_action({
                let editor = editor.clone();
                move |action: &Left, window, cx| {
                    editor.update(cx, |editor, cx| editor.left(action, window, cx))
                }
            })
            .on_action({
                let editor = editor.clone();
                move |action: &Right, window, cx| {
                    editor.update(cx, |editor, cx| editor.right(action, window, cx))
                }
            })
            .on_action({
                let editor = editor.clone();
                move |action: &Home, window, cx| {
                    editor.update(cx, |editor, cx| editor.home(action, window, cx))
                }
            })
            .on_action({
                let editor = editor.clone();
                move |action: &End, window, cx| {
                    editor.update(cx, |editor, cx| editor.end(action, window, cx))
                }
            })
            .on_action({
                let editor = editor.clone();
                move |action: &Backspace, window, cx| {
                    editor.update(cx, |editor, cx| editor.backspace(action, window, cx))
                }
            })
            .on_action({
                let editor = editor.clone();
                move |action: &Delete, window, cx| {
                    editor.update(cx, |editor, cx| editor.delete(action, window, cx))
                }
            })
            .on_action(move |action: &Newline, window, cx| {
                editor.update(cx, |editor, cx| editor.newline(action, window, cx))
            })
    }
}

fn previous_boundary(content: &str, offset: usize) -> usize {
    content
        .grapheme_indices(true)
        .rev()
        .find_map(|(index, _)| (index < offset).then_some(index))
        .unwrap_or(0)
}

fn next_boundary(content: &str, offset: usize) -> usize {
    content
        .grapheme_indices(true)
        .find_map(|(index, _)| (index > offset).then_some(index))
        .unwrap_or(content.len())
}

fn line_start(content: &str, offset: usize) -> usize {
    content[..offset]
        .rfind('\n')
        .map_or(0, |index| index + 1)
}

fn line_end(content: &str, offset: usize) -> usize {
    content[offset..]
        .find('\n')
        .map_or(content.len(), |index| offset + index)
}

fn offset_from_utf16(content: &str, offset: usize) -> usize {
    let mut utf8_offset = 0;
    let mut utf16_count = 0;
    for character in content.chars() {
        if utf16_count >= offset {
            break;
        }
        utf16_count += character.len_utf16();
        utf8_offset += character.len_utf8();
    }
    utf8_offset
}

fn offset_to_utf16(content: &str, offset: usize) -> usize {
    let mut utf16_offset = 0;
    let mut utf8_count = 0;
    for character in content.chars() {
        if utf8_count >= offset {
            break;
        }
        utf8_count += character.len_utf8();
        utf16_offset += character.len_utf16();
    }
    utf16_offset
}

fn range_to_utf16(content: &str, range: &Range<usize>) -> Range<usize> {
    offset_to_utf16(content, range.start)..offset_to_utf16(content, range.end)
}

fn range_from_utf16(content: &str, range: &Range<usize>) -> Range<usize> {
    offset_from_utf16(content, range.start)..offset_from_utf16(content, range.end)
}

fn cursor_line_and_offset(content: &str, cursor: usize) -> (usize, usize) {
    let mut line_index = 0;
    let mut line_start = 0;
    for (index, character) in content.char_indices() {
        if index >= cursor {
            break;
        }
        if character == '\n' {
            line_index += 1;
            line_start = index + 1;
        }
    }
    (line_index, cursor - line_start)
}
