//! Native text editing for Cakify.
//!
//! The shaping and platform input boundary follows GPUI's Apache-2.0 input
//! examples at the workspace-pinned Zed revision. The editing model and
//! multiline behavior are implemented independently for Cakify.

use std::{ops::Range, time::Duration};

use gpui::{
    actions, fill, hsla, point, prelude::*, px, relative, size, App, Bounds, ClipboardItem,
    Context, ElementInputHandler, Entity, EntityInputHandler, FocusHandle, Focusable,
    InteractiveElement, KeyBinding, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, PaintQuad, Pixels, ShapedLine, SharedString, Subscription, Task, TextRun,
    UTF16Selection, UnderlineStyle, Window,
};
use unicode_segmentation::UnicodeSegmentation;

actions!(
    cakify_input,
    [
        Backspace,
        Delete,
        Left,
        Right,
        Up,
        Down,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        Home,
        End,
        SelectHome,
        SelectEnd,
        SelectAll,
        Copy,
        Cut,
        Paste,
        Newline,
        Submit
    ]
);

pub fn bind_input_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, Some("TextInput")),
        KeyBinding::new("delete", Delete, Some("TextInput")),
        KeyBinding::new("left", Left, Some("TextInput")),
        KeyBinding::new("right", Right, Some("TextInput")),
        KeyBinding::new("up", Up, Some("TextInput")),
        KeyBinding::new("down", Down, Some("TextInput")),
        KeyBinding::new("shift-left", SelectLeft, Some("TextInput")),
        KeyBinding::new("shift-right", SelectRight, Some("TextInput")),
        KeyBinding::new("shift-up", SelectUp, Some("TextInput")),
        KeyBinding::new("shift-down", SelectDown, Some("TextInput")),
        KeyBinding::new("home", Home, Some("TextInput")),
        KeyBinding::new("end", End, Some("TextInput")),
        KeyBinding::new("shift-home", SelectHome, Some("TextInput")),
        KeyBinding::new("shift-end", SelectEnd, Some("TextInput")),
        KeyBinding::new("cmd-a", SelectAll, Some("TextInput")),
        KeyBinding::new("cmd-c", Copy, Some("TextInput")),
        KeyBinding::new("cmd-x", Cut, Some("TextInput")),
        KeyBinding::new("cmd-v", Paste, Some("TextInput")),
        KeyBinding::new("shift-enter", Newline, Some("TextInput")),
        KeyBinding::new("enter", Submit, Some("TextInput")),
    ]);
}

pub struct TextEditor {
    value: Entity<String>,
    pub focus_handle: FocusHandle,
    selection: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    preferred_column: Option<usize>,
    cursor_visible: bool,
    placeholder: SharedString,
    masked: bool,
    multiline: bool,
    last_layouts: Vec<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    last_line_height: Option<Pixels>,
    is_selecting: bool,
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
            this.is_selecting = false;
        });
        let value_subscription = cx.observe(&value, |this, value, cx| {
            let content = value.read(cx);
            this.selection = clamp_byte_range(content, this.selection.clone());
            this.marked_range = this
                .marked_range
                .take()
                .map(|range| clamp_byte_range(content, range))
                .filter(|range| !range.is_empty());
            this.preferred_column = None;
            cx.notify();
        });

        Self {
            value,
            focus_handle,
            selection: cursor..cursor,
            selection_reversed: false,
            marked_range: None,
            preferred_column: None,
            cursor_visible: false,
            placeholder: placeholder.into(),
            masked,
            multiline,
            last_layouts: Vec::new(),
            last_bounds: None,
            last_line_height: None,
            is_selecting: false,
            _blink_task: Task::ready(()),
            _subscriptions: vec![focus_subscription, blur_subscription, value_subscription],
        }
    }

    pub fn text(&self, cx: &App) -> String {
        self.value.read(cx).clone()
    }

    pub fn set_text(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        let text = normalize_inserted_text(&text.into(), self.multiline);
        let cursor = text.len();
        self.selection = cursor..cursor;
        self.selection_reversed = false;
        self.marked_range = None;
        self.preferred_column = None;
        self.value.update(cx, |value, cx| {
            *value = text;
            cx.notify();
        });
        cx.notify();
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.set_text(String::new(), cx);
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selection.start
        } else {
            self.selection.end
        }
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

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selection = offset..offset;
        self.selection_reversed = false;
        self.preferred_column = None;
        self.reset_blink(cx);
        cx.notify();
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        let anchor = if self.selection_reversed {
            self.selection.end
        } else {
            self.selection.start
        };
        if offset < anchor {
            self.selection = offset..anchor;
            self.selection_reversed = true;
        } else {
            self.selection = anchor..offset;
            self.selection_reversed = false;
        }
        self.preferred_column = None;
        self.reset_blink(cx);
        cx.notify();
    }

    fn move_vertically(&mut self, delta: isize, selecting: bool, cx: &mut Context<Self>) {
        let content = self.text(cx);
        let cursor = self.cursor_offset();
        let column = self
            .preferred_column
            .unwrap_or_else(|| grapheme_column(&content, cursor));
        let target = vertical_offset(&content, cursor, delta, column);
        if selecting {
            self.select_to(target, cx);
        } else {
            self.move_to(target, cx);
        }
        self.preferred_column = Some(column);
    }

    fn replace_bytes(&mut self, range: Range<usize>, new_text: &str, cx: &mut Context<Self>) {
        let content = self.text(cx);
        let range = clamp_byte_range(&content, range);
        let replacement = splice_text(&content, range.clone(), new_text);
        let cursor = range.start + new_text.len();
        self.selection = cursor..cursor;
        self.selection_reversed = false;
        self.marked_range = None;
        self.preferred_column = None;
        self.value.update(cx, |value, cx| {
            *value = replacement;
            cx.notify();
        });
        self.reset_blink(cx);
        cx.notify();
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        let content = self.text(cx);
        let offset = if self.selection.is_empty() {
            previous_boundary(&content, self.cursor_offset())
        } else {
            self.selection.start
        };
        self.move_to(offset, cx);
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        let content = self.text(cx);
        let offset = if self.selection.is_empty() {
            next_boundary(&content, self.cursor_offset())
        } else {
            self.selection.end
        };
        self.move_to(offset, cx);
    }

    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertically(-1, false, cx);
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertically(1, false, cx);
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        let content = self.text(cx);
        self.select_to(previous_boundary(&content, self.cursor_offset()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        let content = self.text(cx);
        self.select_to(next_boundary(&content, self.cursor_offset()), cx);
    }

    fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertically(-1, true, cx);
    }

    fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        self.move_vertically(1, true, cx);
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        let content = self.text(cx);
        self.move_to(line_start(&content, self.cursor_offset()), cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        let content = self.text(cx);
        self.move_to(line_end(&content, self.cursor_offset()), cx);
    }

    fn select_home(&mut self, _: &SelectHome, _: &mut Window, cx: &mut Context<Self>) {
        let content = self.text(cx);
        self.select_to(line_start(&content, self.cursor_offset()), cx);
    }

    fn select_end(&mut self, _: &SelectEnd, _: &mut Window, cx: &mut Context<Self>) {
        let content = self.text(cx);
        self.select_to(line_end(&content, self.cursor_offset()), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        let end = self.text(cx).len();
        self.selection = 0..end;
        self.selection_reversed = false;
        self.preferred_column = None;
        self.reset_blink(cx);
        cx.notify();
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selection.is_empty() {
            let content = self.text(cx);
            let cursor = self.cursor_offset();
            let previous = previous_boundary(&content, cursor);
            if previous == cursor {
                window.play_system_bell();
                return;
            }
            self.selection = previous..cursor;
        }
        self.replace_bytes(self.selection.clone(), "", cx);
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selection.is_empty() {
            let content = self.text(cx);
            let cursor = self.cursor_offset();
            let next = next_boundary(&content, cursor);
            if next == cursor {
                window.play_system_bell();
                return;
            }
            self.selection = cursor..next;
        }
        self.replace_bytes(self.selection.clone(), "", cx);
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selection.is_empty() {
            let content = self.text(cx);
            cx.write_to_clipboard(ClipboardItem::new_string(
                content[self.selection.clone()].to_owned(),
            ));
        }
    }

    fn cut(&mut self, _: &Cut, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selection.is_empty() {
            let content = self.text(cx);
            cx.write_to_clipboard(ClipboardItem::new_string(
                content[self.selection.clone()].to_owned(),
            ));
            self.replace_bytes(self.selection.clone(), "", cx);
        }
    }

    fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            let text = normalize_inserted_text(&text, self.multiline);
            self.replace_bytes(self.selection.clone(), &text, cx);
        }
    }

    fn newline(&mut self, _: &Newline, _: &mut Window, cx: &mut Context<Self>) {
        if self.multiline {
            self.replace_bytes(self.selection.clone(), "\n", cx);
        }
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle, cx);
        self.is_selecting = true;
        let content = self.text(cx);
        let offset = self.byte_index_for_point(&content, event.position);
        if event.modifiers.shift {
            self.select_to(offset, cx);
        } else {
            self.move_to(offset, cx);
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            let content = self.text(cx);
            let offset = self.byte_index_for_point(&content, event.position);
            self.select_to(offset, cx);
        }
    }

    fn byte_index_for_point(&self, content: &str, position: gpui::Point<Pixels>) -> usize {
        let (Some(bounds), Some(line_height)) = (self.last_bounds, self.last_line_height) else {
            return self.cursor_offset();
        };
        if self.last_layouts.is_empty() {
            return 0;
        }
        let line_index = line_index_for_y(position.y, bounds, line_height, self.last_layouts.len());
        let ranges = line_ranges(content);
        let range = ranges
            .get(line_index)
            .cloned()
            .unwrap_or(content.len()..content.len());
        let line = &self.last_layouts[line_index.min(self.last_layouts.len() - 1)];
        let display_offset = line.closest_index_for_x(position.x - bounds.left());
        let source_offset = if self.masked {
            display_to_source_offset(&content[range.clone()], display_offset)
        } else {
            display_offset.min(range.len())
        };
        range.start + source_offset
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
        Some(UTF16Selection {
            range: range_to_utf16(&content, &self.selection),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        let content = self.text(cx);
        self.marked_range
            .as_ref()
            .map(|range| range_to_utf16(&content, range))
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.marked_range = None;
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let content = self.text(cx);
        let range = range_utf16
            .as_ref()
            .map(|range| range_from_utf16(&content, range))
            .or_else(|| self.marked_range.clone())
            .unwrap_or_else(|| self.selection.clone());
        let new_text = normalize_inserted_text(new_text, self.multiline);
        self.replace_bytes(range, &new_text, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let content = self.text(cx);
        let range = range_utf16
            .as_ref()
            .map(|range| range_from_utf16(&content, range))
            .or_else(|| self.marked_range.clone())
            .unwrap_or_else(|| self.selection.clone());
        let range = clamp_byte_range(&content, range);
        let new_text = normalize_inserted_text(new_text, self.multiline);
        let replacement = splice_text(&content, range.clone(), &new_text);
        let inserted = range.start..range.start + new_text.len();
        let relative_selection = new_selected_range_utf16
            .as_ref()
            .map(|selection| range_from_utf16(&new_text, selection))
            .unwrap_or(new_text.len()..new_text.len());
        self.selection =
            range.start + relative_selection.start..range.start + relative_selection.end;
        self.selection_reversed = false;
        self.marked_range = (!inserted.is_empty()).then_some(inserted);
        self.preferred_column = None;
        self.value.update(cx, |value, cx| {
            *value = replacement;
            cx.notify();
        });
        self.reset_blink(cx);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let line_height = self.last_line_height?;
        if self.last_layouts.is_empty() {
            return None;
        }
        let content = self.text(cx);
        let range = range_from_utf16(&content, &range_utf16);
        let (start_line, start_offset) = line_and_offset(&content, range.start);
        let (end_line, end_offset) = line_and_offset(&content, range.end);
        let start_layout = self.last_layouts.get(start_line)?;
        let end_layout = self.last_layouts.get(end_line)?;
        let ranges = line_ranges(&content);
        let start_source = &content[ranges.get(start_line)?.clone()];
        let end_source = &content[ranges.get(end_line)?.clone()];
        let start_offset = if self.masked {
            source_to_display_offset(start_source, start_offset)
        } else {
            start_offset
        };
        let end_offset = if self.masked {
            source_to_display_offset(end_source, end_offset)
        } else {
            end_offset
        };
        let start = point(
            bounds.left() + start_layout.x_for_index(start_offset),
            bounds.top() + line_height * start_line as f32,
        );
        if start_line == end_line {
            return Some(Bounds::from_corners(
                start,
                point(
                    bounds.left() + end_layout.x_for_index(end_offset),
                    start.y + line_height,
                ),
            ));
        }
        Some(Bounds::from_corners(
            point(bounds.left(), start.y),
            point(
                bounds.right(),
                bounds.top() + line_height * (end_line + 1) as f32,
            ),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: gpui::Point<Pixels>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<usize> {
        let content = self.text(cx);
        Some(offset_to_utf16(
            &content,
            self.byte_index_for_point(&content, point),
        ))
    }

    fn set_selected_text_range(
        &mut self,
        range_utf16: Range<usize>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let content = self.text(cx);
        self.selection = range_from_utf16(&content, &range_utf16);
        self.selection_reversed = false;
        self.preferred_column = None;
        self.reset_blink(cx);
        cx.notify();
    }

    fn text_length_utf16(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> Option<usize> {
        Some(self.text(cx).encode_utf16().count())
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
    selections: Vec<PaintQuad>,
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
        let is_focused = editor.focus_handle.is_focused(window);
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line_height = window.line_height();
        let is_placeholder = content.is_empty();
        let ranges = line_ranges(&content);

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
            ranges
                .iter()
                .map(|range| {
                    let source = &content[range.clone()];
                    let display = display_text(source, editor.masked);
                    let text: SharedString = display.into();
                    let base_run = TextRun {
                        len: text.len(),
                        font: style.font(),
                        color: style.color,
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    };
                    let runs = marked_runs(
                        base_run,
                        range,
                        editor.marked_range.as_ref(),
                        source,
                        editor.masked,
                    );
                    window
                        .text_system()
                        .shape_line(text, font_size, &runs, None)
                })
                .collect::<Vec<_>>()
        };

        let mut selections = Vec::new();
        if is_focused && !editor.selection.is_empty() && !is_placeholder {
            for (line_index, range) in ranges.iter().enumerate() {
                let Some(intersection) = range_intersection(range, &editor.selection) else {
                    continue;
                };
                let source = &content[range.clone()];
                let start = intersection.start - range.start;
                let end = intersection.end - range.start;
                let start = if editor.masked {
                    source_to_display_offset(source, start)
                } else {
                    start
                };
                let end = if editor.masked {
                    source_to_display_offset(source, end)
                } else {
                    end
                };
                let selects_newline = end == start
                    && line_index + 1 < ranges.len()
                    && editor.selection.start <= range.end
                    && editor.selection.end > range.end;
                if end > start || selects_newline {
                    let start_x = lines[line_index].x_for_index(start);
                    let end_x = if selects_newline {
                        start_x + px(6.)
                    } else {
                        lines[line_index].x_for_index(end)
                    };
                    selections.push(fill(
                        Bounds::from_corners(
                            point(
                                bounds.left() + start_x,
                                bounds.top() + line_height * line_index as f32,
                            ),
                            point(
                                bounds.left() + end_x,
                                bounds.top() + line_height * (line_index + 1) as f32,
                            ),
                        ),
                        hsla(159. / 360., 0.55, 0.42, 0.24),
                    ));
                }
            }
        }

        let cursor = if is_focused && editor.cursor_visible && editor.selection.is_empty() {
            let cursor_offset = editor.cursor_offset();
            let (cursor_line, offset_in_line) = line_and_offset(&content, cursor_offset);
            let cursor_line = cursor_line.min(lines.len().saturating_sub(1));
            let source = ranges
                .get(cursor_line)
                .map(|range| &content[range.clone()])
                .unwrap_or("");
            let offset_in_line = if editor.masked {
                source_to_display_offset(source, offset_in_line)
            } else {
                offset_in_line
            };
            let cursor_x = if is_placeholder {
                px(0.)
            } else {
                lines[cursor_line].x_for_index(offset_in_line)
            };
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

        EditorTextPrepaint {
            lines,
            cursor,
            selections,
        }
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
        for selection in prepaint.selections.drain(..) {
            window.paint_quad(selection);
        }
        let line_height = window.line_height();
        let lines = std::mem::take(&mut prepaint.lines);
        for (index, line) in lines.iter().enumerate() {
            let origin = point(bounds.left(), bounds.top() + line_height * index as f32);
            line.paint(origin, line_height, gpui::TextAlign::Left, None, window, cx)
                .expect("paint editor text");
        }
        if let Some(cursor) = prepaint.cursor.take() {
            window.paint_quad(cursor);
        }
        self.editor.update(cx, |editor, _cx| {
            editor.last_layouts = lines;
            editor.last_bounds = Some(bounds);
            editor.last_line_height = Some(line_height);
        });
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
                move |action: &Up, window, cx| {
                    editor.update(cx, |editor, cx| editor.up(action, window, cx))
                }
            })
            .on_action({
                let editor = editor.clone();
                move |action: &Down, window, cx| {
                    editor.update(cx, |editor, cx| editor.down(action, window, cx))
                }
            })
            .on_action({
                let editor = editor.clone();
                move |action: &SelectLeft, window, cx| {
                    editor.update(cx, |editor, cx| editor.select_left(action, window, cx))
                }
            })
            .on_action({
                let editor = editor.clone();
                move |action: &SelectRight, window, cx| {
                    editor.update(cx, |editor, cx| editor.select_right(action, window, cx))
                }
            })
            .on_action({
                let editor = editor.clone();
                move |action: &SelectUp, window, cx| {
                    editor.update(cx, |editor, cx| editor.select_up(action, window, cx))
                }
            })
            .on_action({
                let editor = editor.clone();
                move |action: &SelectDown, window, cx| {
                    editor.update(cx, |editor, cx| editor.select_down(action, window, cx))
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
                move |action: &SelectHome, window, cx| {
                    editor.update(cx, |editor, cx| editor.select_home(action, window, cx))
                }
            })
            .on_action({
                let editor = editor.clone();
                move |action: &SelectEnd, window, cx| {
                    editor.update(cx, |editor, cx| editor.select_end(action, window, cx))
                }
            })
            .on_action({
                let editor = editor.clone();
                move |action: &SelectAll, window, cx| {
                    editor.update(cx, |editor, cx| editor.select_all(action, window, cx))
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
            .on_action({
                let editor = editor.clone();
                move |action: &Copy, window, cx| {
                    editor.update(cx, |editor, cx| editor.copy(action, window, cx))
                }
            })
            .on_action({
                let editor = editor.clone();
                move |action: &Cut, window, cx| {
                    editor.update(cx, |editor, cx| editor.cut(action, window, cx))
                }
            })
            .on_action({
                let editor = editor.clone();
                move |action: &Paste, window, cx| {
                    editor.update(cx, |editor, cx| editor.paste(action, window, cx))
                }
            })
            .on_action({
                let editor = editor.clone();
                move |action: &Newline, window, cx| {
                    editor.update(cx, |editor, cx| editor.newline(action, window, cx))
                }
            })
            .on_mouse_down(MouseButton::Left, {
                let editor = editor.clone();
                move |event, window, cx| {
                    editor.update(cx, |editor, cx| editor.on_mouse_down(event, window, cx))
                }
            })
            .on_mouse_move({
                let editor = editor.clone();
                move |event, window, cx| {
                    editor.update(cx, |editor, cx| editor.on_mouse_move(event, window, cx))
                }
            })
            .on_mouse_up(MouseButton::Left, {
                let editor = editor.clone();
                move |event, window, cx| {
                    editor.update(cx, |editor, cx| editor.on_mouse_up(event, window, cx))
                }
            })
            .on_mouse_up_out(MouseButton::Left, move |event, window, cx| {
                editor.update(cx, |editor, cx| editor.on_mouse_up(event, window, cx))
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
    content[..offset].rfind('\n').map_or(0, |index| index + 1)
}

fn line_end(content: &str, offset: usize) -> usize {
    content[offset..]
        .find('\n')
        .map_or(content.len(), |index| offset + index)
}

fn grapheme_column(content: &str, offset: usize) -> usize {
    content[line_start(content, offset)..offset]
        .graphemes(true)
        .count()
}

fn vertical_offset(content: &str, offset: usize, delta: isize, column: usize) -> usize {
    let ranges = line_ranges(content);
    let (line, _) = line_and_offset(content, offset);
    let target_line = line
        .saturating_add_signed(delta)
        .min(ranges.len().saturating_sub(1));
    let range = ranges[target_line].clone();
    content[range.clone()]
        .grapheme_indices(true)
        .nth(column)
        .map_or(range.end, |(index, _)| range.start + index)
}

fn line_ranges(content: &str) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = 0;
    for (index, character) in content.char_indices() {
        if character == '\n' {
            ranges.push(start..index);
            start = index + 1;
        }
    }
    ranges.push(start..content.len());
    ranges
}

fn line_and_offset(content: &str, cursor: usize) -> (usize, usize) {
    let mut line_index = 0;
    let mut start = 0;
    for (index, character) in content.char_indices() {
        if index >= cursor {
            break;
        }
        if character == '\n' {
            line_index += 1;
            start = index + 1;
        }
    }
    (line_index, cursor - start)
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
    let offset = floor_char_boundary(content, offset.min(content.len()));
    content[..offset].encode_utf16().count()
}

fn range_to_utf16(content: &str, range: &Range<usize>) -> Range<usize> {
    offset_to_utf16(content, range.start)..offset_to_utf16(content, range.end)
}

fn range_from_utf16(content: &str, range: &Range<usize>) -> Range<usize> {
    let start = offset_from_utf16(content, range.start);
    let end = offset_from_utf16(content, range.end);
    start.min(end)..start.max(end)
}

fn floor_char_boundary(content: &str, offset: usize) -> usize {
    let mut offset = offset.min(content.len());
    while offset > 0 && !content.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

fn clamp_byte_range(content: &str, range: Range<usize>) -> Range<usize> {
    let start = floor_char_boundary(content, range.start);
    let end = floor_char_boundary(content, range.end);
    start.min(end)..start.max(end)
}

fn normalize_inserted_text(text: &str, multiline: bool) -> String {
    if multiline {
        text.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        text.replace(['\r', '\n'], "")
    }
}

fn splice_text(content: &str, range: Range<usize>, new_text: &str) -> String {
    let mut result = String::with_capacity(content.len() - range.len() + new_text.len());
    result.push_str(&content[..range.start]);
    result.push_str(new_text);
    result.push_str(&content[range.end..]);
    result
}

fn display_text(source: &str, masked: bool) -> String {
    if masked {
        source.chars().map(|_| '*').collect()
    } else {
        source.to_owned()
    }
}

fn source_to_display_offset(source: &str, source_offset: usize) -> usize {
    source[..floor_char_boundary(source, source_offset)]
        .chars()
        .count()
}

fn display_to_source_offset(source: &str, display_offset: usize) -> usize {
    source
        .char_indices()
        .nth(display_offset)
        .map_or(source.len(), |(index, _)| index)
}

fn range_intersection(left: &Range<usize>, right: &Range<usize>) -> Option<Range<usize>> {
    let start = left.start.max(right.start);
    let end = left.end.min(right.end);
    (start <= end).then_some(start..end)
}

fn marked_runs(
    base: TextRun,
    line_range: &Range<usize>,
    marked_range: Option<&Range<usize>>,
    source: &str,
    masked: bool,
) -> Vec<TextRun> {
    let Some(marked) = marked_range.and_then(|marked| range_intersection(line_range, marked))
    else {
        return vec![base];
    };
    if marked.is_empty() {
        return vec![base];
    }
    let start = marked.start - line_range.start;
    let end = marked.end - line_range.start;
    let start = if masked {
        source_to_display_offset(source, start)
    } else {
        start
    };
    let end = if masked {
        source_to_display_offset(source, end)
    } else {
        end
    };
    [
        TextRun {
            len: start,
            ..base.clone()
        },
        TextRun {
            len: end - start,
            underline: Some(UnderlineStyle {
                color: Some(base.color),
                thickness: px(1.),
                wavy: false,
            }),
            ..base.clone()
        },
        TextRun {
            len: base.len - end,
            ..base
        },
    ]
    .into_iter()
    .filter(|run| run.len > 0)
    .collect()
}

fn line_index_for_y(
    y: Pixels,
    bounds: Bounds<Pixels>,
    line_height: Pixels,
    line_count: usize,
) -> usize {
    if y <= bounds.top() {
        return 0;
    }
    if y >= bounds.bottom() {
        return line_count.saturating_sub(1);
    }
    (((y - bounds.top()) / line_height) as usize).min(line_count.saturating_sub(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unicode_boundaries_never_split_a_grapheme() {
        let content = "a👨‍👩‍👧‍👦好";
        let family_start = 1;
        let family_end = next_boundary(content, family_start);
        assert_eq!(previous_boundary(content, family_end), family_start);
        assert_eq!(next_boundary(content, family_end), content.len());
    }

    #[test]
    fn utf16_ranges_are_adjusted_to_scalar_boundaries() {
        let content = "a😀b";
        assert_eq!(range_from_utf16(content, &(1..2)), 1..5);
        assert_eq!(range_to_utf16(content, &(1..5)), 1..3);
        assert_eq!(offset_to_utf16(content, content.len()), 4);
    }

    #[test]
    fn vertical_movement_preserves_grapheme_column() {
        let content = "ab好\nx\n1234";
        let first_line_end = line_end(content, 0);
        let second = vertical_offset(content, first_line_end, 1, 3);
        assert_eq!(second, line_end(content, second));
        let third = vertical_offset(content, second, 1, 3);
        assert_eq!(&content[third..], "4");
    }

    #[test]
    fn windows_newlines_are_normalized_without_flattening_multiline_paste() {
        assert_eq!(normalize_inserted_text("a\r\nb\rc", true), "a\nb\nc");
        assert_eq!(normalize_inserted_text("a\r\nb\nc", false), "abc");
    }

    #[test]
    fn splice_replaces_the_selection_atomically() {
        assert_eq!(splice_text("hello 世界", 6..12, "GPUI"), "hello GPUI");
        assert_eq!(line_ranges("a\n"), vec![0..1, 2..2]);
    }

    #[test]
    fn masked_offsets_round_trip_across_multibyte_text() {
        let source = "a😀好";
        for source_offset in [0, 1, 5, source.len()] {
            let display = source_to_display_offset(source, source_offset);
            assert_eq!(display_to_source_offset(source, display), source_offset);
        }
    }
}
