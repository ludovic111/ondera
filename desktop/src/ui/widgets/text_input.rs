//! A text field: one line (names, search, numbers) or several (the agent's message box),
//! with selection, the clipboard, word and line moves, and IME composition. Built on GPUI's
//! input handler, after its `input.rs` example.
//!
//! The owner keeps the `Entity<TextInput>` and subscribes to [`InputEvent`]: `Changed` on
//! every edit, `Submit` on Enter (Shift+Enter is a new line in a multi-line field),
//! `Cancel` on Escape, `Blur` when focus leaves.

use crate::ui::theme::{radius, size, Theme, FONT_UI};
use gpui::{
    actions, div, point, prelude::*, px, relative, App, Bounds, ClipboardItem, Context,
    CursorStyle, ElementId, ElementInputHandler, Entity, EntityInputHandler, EventEmitter,
    FocusHandle, Focusable, GlobalElementId, KeyBinding, LayoutId, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point, SharedString, Style, TextRun,
    UTF16Selection, UnderlineStyle, Window, WrappedLine,
};
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

actions!(
    text_input,
    [
        Backspace,
        Delete,
        DeleteWordLeft,
        Left,
        Right,
        Up,
        Down,
        WordLeft,
        WordRight,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        SelectWordLeft,
        SelectWordRight,
        SelectAll,
        Home,
        End,
        SelectHome,
        SelectEnd,
        Enter,
        Newline,
        Escape,
        Paste,
        Cut,
        Copy,
        ShowCharacterPalette,
    ]
);

/// Text-field keys, active only while a field has focus.
pub fn bind(cx: &mut App) {
    let c = Some("TextInput");
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, c),
        KeyBinding::new("delete", Delete, c),
        KeyBinding::new("alt-backspace", DeleteWordLeft, c),
        KeyBinding::new("left", Left, c),
        KeyBinding::new("right", Right, c),
        KeyBinding::new("up", Up, c),
        KeyBinding::new("down", Down, c),
        KeyBinding::new("alt-left", WordLeft, c),
        KeyBinding::new("alt-right", WordRight, c),
        KeyBinding::new("shift-left", SelectLeft, c),
        KeyBinding::new("shift-right", SelectRight, c),
        KeyBinding::new("shift-up", SelectUp, c),
        KeyBinding::new("shift-down", SelectDown, c),
        KeyBinding::new("alt-shift-left", SelectWordLeft, c),
        KeyBinding::new("alt-shift-right", SelectWordRight, c),
        KeyBinding::new("secondary-a", SelectAll, c),
        KeyBinding::new("home", Home, c),
        KeyBinding::new("end", End, c),
        KeyBinding::new("cmd-left", Home, c),
        KeyBinding::new("cmd-right", End, c),
        KeyBinding::new("shift-home", SelectHome, c),
        KeyBinding::new("shift-end", SelectEnd, c),
        KeyBinding::new("cmd-shift-left", SelectHome, c),
        KeyBinding::new("cmd-shift-right", SelectEnd, c),
        KeyBinding::new("enter", Enter, c),
        KeyBinding::new("shift-enter", Newline, c),
        KeyBinding::new("escape", Escape, c),
        KeyBinding::new("secondary-v", Paste, c),
        KeyBinding::new("secondary-c", Copy, c),
        KeyBinding::new("secondary-x", Cut, c),
        KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, c),
    ]);
}

#[derive(Clone, Debug, PartialEq)]
pub enum InputEvent {
    Changed,
    Submit,
    Cancel,
    Blur,
}

pub struct TextInput {
    focus: FocusHandle,
    content: String,
    placeholder: SharedString,
    multiline: bool,
    /// Monospaced digits for numbers and paths.
    mono: bool,
    selected: Range<usize>,
    reversed: bool,
    marked: Option<Range<usize>>,
    lines: Vec<WrappedLine>,
    /// Byte offset where each laid-out hard line starts.
    starts: Vec<usize>,
    bounds: Option<Bounds<Pixels>>,
    line_height: Pixels,
    selecting: bool,
    /// The column a vertical move keeps.
    goal_x: Option<Pixels>,
    was_focused: bool,
}

impl EventEmitter<InputEvent> for TextInput {}

impl TextInput {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus: cx.focus_handle(),
            content: String::new(),
            placeholder: SharedString::default(),
            multiline: false,
            mono: false,
            selected: 0..0,
            reversed: false,
            marked: None,
            lines: vec![],
            starts: vec![],
            bounds: None,
            line_height: px(18.0),
            selecting: false,
            goal_x: None,
            was_focused: false,
        }
    }
    pub fn multiline(mut self) -> Self {
        self.multiline = true;
        self
    }
    pub fn mono(mut self) -> Self {
        self.mono = true;
        self
    }
    pub fn placeholder(mut self, text: impl Into<SharedString>) -> Self {
        self.placeholder = text.into();
        self
    }

    pub fn text(&self) -> &str {
        &self.content
    }
    /// Replace the text (from the host), keeping the caret at the end. Does not emit.
    pub fn set_text(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        let text = text.into();
        if text != self.content {
            self.content = text;
            self.selected = self.content.len()..self.content.len();
            self.marked = None;
            cx.notify();
        }
    }
    pub fn set_placeholder(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.placeholder = text.into();
        cx.notify();
    }
    pub fn select_all_text(&mut self, cx: &mut Context<Self>) {
        self.selected = 0..self.content.len();
        self.reversed = false;
        cx.notify();
    }
    pub fn is_focused(&self, window: &Window) -> bool {
        self.focus.is_focused(window)
    }
    pub fn focus(&self, window: &mut Window) {
        window.focus(&self.focus);
    }

    fn cursor(&self) -> usize {
        if self.reversed {
            self.selected.start
        } else {
            self.selected.end
        }
    }
    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected = offset..offset;
        self.reversed = false;
        cx.notify();
    }
    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        if self.reversed {
            self.selected.start = offset;
        } else {
            self.selected.end = offset;
        }
        if self.selected.end < self.selected.start {
            self.reversed = !self.reversed;
            self.selected = self.selected.end..self.selected.start;
        }
        cx.notify();
    }
    fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(i, _)| (i < offset).then_some(i))
            .unwrap_or(0)
    }
    fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(i, _)| (i > offset).then_some(i))
            .unwrap_or(self.content.len())
    }
    fn previous_word(&self, offset: usize) -> usize {
        let mut last = 0;
        for (i, w) in self.content.split_word_bound_indices() {
            if i >= offset {
                break;
            }
            if !w.trim().is_empty() {
                last = i;
            }
        }
        last
    }
    fn next_word(&self, offset: usize) -> usize {
        for (i, w) in self.content.split_word_bound_indices() {
            let end = i + w.len();
            if end > offset && !w.trim().is_empty() {
                return end;
            }
        }
        self.content.len()
    }
    fn line_start(&self, offset: usize) -> usize {
        self.content[..offset].rfind('\n').map_or(0, |i| i + 1)
    }
    fn line_end(&self, offset: usize) -> usize {
        self.content[offset..]
            .find('\n')
            .map_or(self.content.len(), |i| offset + i)
    }

    /// Where an offset is drawn, relative to the field's origin.
    fn position_of(&self, offset: usize) -> Option<Point<Pixels>> {
        let mut y = px(0.0);
        for (line, start) in self.lines.iter().zip(&self.starts) {
            let end = start + line.len();
            if offset >= *start && offset <= end {
                let p = line.position_for_index(offset - start, self.line_height)?;
                return Some(point(p.x, p.y + y));
            }
            y += line.size(self.line_height).height.max(self.line_height);
        }
        None
    }
    /// The offset under a point relative to the field's origin.
    fn offset_at(&self, position: Point<Pixels>) -> usize {
        let mut y = px(0.0);
        for (i, (line, start)) in self.lines.iter().zip(&self.starts).enumerate() {
            let h = line.size(self.line_height).height.max(self.line_height);
            if position.y < y + h || i + 1 == self.lines.len() {
                let local = point(position.x.max(px(0.0)), (position.y - y).max(px(0.0)));
                let index = match line.closest_index_for_position(local, self.line_height) {
                    Ok(i) | Err(i) => i,
                };
                return start + index;
            }
            y += h;
        }
        self.content.len()
    }
    fn vertical(&mut self, down: bool, select: bool, cx: &mut Context<Self>) {
        let cursor = self.cursor();
        let Some(p) = self.position_of(cursor) else {
            return;
        };
        let x = *self.goal_x.get_or_insert(p.x);
        let y = if down {
            p.y + self.line_height * 1.5
        } else {
            p.y - self.line_height * 0.5
        };
        let target = if y < px(0.0) {
            0
        } else {
            self.offset_at(point(x, y))
        };
        if select {
            self.select_to(target, cx);
        } else {
            self.move_to(target, cx);
        }
        self.goal_x = Some(x);
    }

    fn edit(&mut self, range: Range<usize>, text: &str, cx: &mut Context<Self>) {
        let text = if self.multiline {
            text.to_string()
        } else {
            text.replace(['\n', '\r'], " ")
        };
        self.content.replace_range(range.clone(), &text);
        let caret = range.start + text.len();
        self.selected = caret..caret;
        self.reversed = false;
        self.marked = None;
        self.goal_x = None;
        cx.emit(InputEvent::Changed);
        cx.notify();
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        self.goal_x = None;
        if self.selected.is_empty() {
            self.move_to(self.previous_boundary(self.cursor()), cx);
        } else {
            self.move_to(self.selected.start, cx);
        }
    }
    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        self.goal_x = None;
        if self.selected.is_empty() {
            self.move_to(self.next_boundary(self.selected.end), cx);
        } else {
            self.move_to(self.selected.end, cx);
        }
    }
    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        if self.multiline {
            self.vertical(false, false, cx);
        } else {
            self.move_to(0, cx);
        }
    }
    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        if self.multiline {
            self.vertical(true, false, cx);
        } else {
            self.move_to(self.content.len(), cx);
        }
    }
    fn word_left(&mut self, _: &WordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.previous_word(self.cursor()), cx);
    }
    fn word_right(&mut self, _: &WordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.next_word(self.cursor()), cx);
    }
    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor()), cx);
    }
    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor()), cx);
    }
    fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical(false, true, cx);
    }
    fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical(true, true, cx);
    }
    fn select_word_left(&mut self, _: &SelectWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_word(self.cursor()), cx);
    }
    fn select_word_right(&mut self, _: &SelectWordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_word(self.cursor()), cx);
    }
    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.select_all_text(cx);
    }
    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        let target = if self.multiline {
            self.line_start(self.cursor())
        } else {
            0
        };
        self.move_to(target, cx);
    }
    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        let target = if self.multiline {
            self.line_end(self.cursor())
        } else {
            self.content.len()
        };
        self.move_to(target, cx);
    }
    fn select_home(&mut self, _: &SelectHome, _: &mut Window, cx: &mut Context<Self>) {
        let target = if self.multiline {
            self.line_start(self.cursor())
        } else {
            0
        };
        self.select_to(target, cx);
    }
    fn select_end(&mut self, _: &SelectEnd, _: &mut Window, cx: &mut Context<Self>) {
        let target = if self.multiline {
            self.line_end(self.cursor())
        } else {
            self.content.len()
        };
        self.select_to(target, cx);
    }
    fn backspace(&mut self, _: &Backspace, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected.is_empty() {
            self.select_to(self.previous_boundary(self.cursor()), cx);
        }
        self.edit(self.selected.clone(), "", cx);
    }
    fn delete(&mut self, _: &Delete, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected.is_empty() {
            self.select_to(self.next_boundary(self.cursor()), cx);
        }
        self.edit(self.selected.clone(), "", cx);
    }
    fn delete_word_left(&mut self, _: &DeleteWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected.is_empty() {
            self.select_to(self.previous_word(self.cursor()), cx);
        }
        self.edit(self.selected.clone(), "", cx);
    }
    fn enter(&mut self, _: &Enter, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(InputEvent::Submit);
    }
    fn newline(&mut self, _: &Newline, _: &mut Window, cx: &mut Context<Self>) {
        if self.multiline {
            self.edit(self.selected.clone(), "\n", cx);
        } else {
            cx.emit(InputEvent::Submit);
        }
    }
    fn escape(&mut self, _: &Escape, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(InputEvent::Cancel);
    }
    fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.edit(self.selected.clone(), &text, cx);
        }
    }
    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected.clone()].to_string(),
            ));
        }
    }
    fn cut(&mut self, _: &Cut, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected.clone()].to_string(),
            ));
            self.edit(self.selected.clone(), "", cx);
        }
    }
    fn show_character_palette(
        &mut self,
        _: &ShowCharacterPalette,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.show_character_palette();
    }

    fn mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus);
        let Some(bounds) = self.bounds else {
            return;
        };
        let offset = self.offset_at(event.position - bounds.origin);
        self.goal_x = None;
        if event.click_count >= 3 {
            self.selected = 0..self.content.len();
            cx.notify();
        } else if event.click_count == 2 {
            let start = self.previous_word(offset.min(self.next_word(offset)));
            let end = self.next_word(start);
            self.selected = start..end;
            self.reversed = false;
            cx.notify();
        } else {
            self.selecting = true;
            if event.modifiers.shift {
                self.select_to(offset, cx);
            } else {
                self.move_to(offset, cx);
            }
        }
    }
    fn mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.selecting = false;
    }
    fn mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.selecting {
            if let Some(bounds) = self.bounds {
                self.select_to(self.offset_at(event.position - bounds.origin), cx);
            }
        }
    }

    fn utf16_to_utf8(&self, offset: usize) -> usize {
        let mut utf8 = 0;
        let mut utf16 = 0;
        for ch in self.content.chars() {
            if utf16 >= offset {
                break;
            }
            utf16 += ch.len_utf16();
            utf8 += ch.len_utf8();
        }
        utf8
    }
    fn utf8_to_utf16(&self, offset: usize) -> usize {
        let mut utf16 = 0;
        let mut utf8 = 0;
        for ch in self.content.chars() {
            if utf8 >= offset {
                break;
            }
            utf8 += ch.len_utf8();
            utf16 += ch.len_utf16();
        }
        utf16
    }
    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.utf8_to_utf16(range.start)..self.utf8_to_utf16(range.end)
    }
    fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.utf16_to_utf8(range.start)..self.utf16_to_utf8(range.end)
    }
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected),
            reversed: self.reversed,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked.as_ref().map(|r| self.range_to_utf16(r))
    }
    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.marked = None;
    }
    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|r| self.range_from_utf16(r))
            .or(self.marked.clone())
            .unwrap_or(self.selected.clone());
        self.edit(range, text, cx);
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        text: &str,
        new_selected: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|r| self.range_from_utf16(r))
            .or(self.marked.clone())
            .unwrap_or(self.selected.clone());
        self.content.replace_range(range.clone(), text);
        self.marked = (!text.is_empty()).then(|| range.start..range.start + text.len());
        self.selected = new_selected
            .as_ref()
            .map(|r| self.range_from_utf16(r))
            .map(|r| r.start + range.start..r.end + range.start)
            .unwrap_or_else(|| range.start + text.len()..range.start + text.len());
        cx.emit(InputEvent::Changed);
        cx.notify();
    }
    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let range = self.range_from_utf16(&range_utf16);
        let start = self.position_of(range.start)?;
        let end = self.position_of(range.end)?;
        Some(Bounds::from_corners(
            bounds.origin + start,
            bounds.origin + point(end.x, end.y + self.line_height),
        ))
    }
    fn character_index_for_point(
        &mut self,
        p: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let bounds = self.bounds?;
        Some(self.utf8_to_utf16(self.offset_at(p - bounds.origin)))
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

/// Paints the text, the selection and the caret; lays the text out at the field's width.
struct TextElement {
    input: Entity<TextInput>,
}

impl IntoElement for TextElement {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

struct Laid {
    lines: Vec<WrappedLine>,
    starts: Vec<usize>,
}

fn shape(
    input: &TextInput,
    width: Option<Pixels>,
    window: &mut Window,
    placeholder_color: gpui::Hsla,
) -> Laid {
    let style = window.text_style();
    let font_size = style.font_size.to_pixels(window.rem_size());
    let (text, color) = if input.content.is_empty() {
        (input.placeholder.to_string(), placeholder_color)
    } else {
        (input.content.clone(), style.color)
    };
    let mut lines = vec![];
    let mut starts = vec![];
    let mut start = 0;
    let hard: Vec<&str> = if input.multiline {
        text.split('\n').collect()
    } else {
        vec![text.as_str()]
    };
    for line in hard {
        let run = TextRun {
            len: line.len(),
            font: style.font(),
            color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let mut runs = vec![run.clone()];
        if let Some(marked) = input.marked.as_ref().filter(|_| !input.content.is_empty()) {
            let (a, b) = (
                marked.start.clamp(start, start + line.len()) - start,
                marked.end.clamp(start, start + line.len()) - start,
            );
            if b > a {
                runs = vec![
                    TextRun {
                        len: a,
                        ..run.clone()
                    },
                    TextRun {
                        len: b - a,
                        underline: Some(UnderlineStyle {
                            color: Some(color),
                            thickness: px(1.0),
                            wavy: false,
                        }),
                        ..run.clone()
                    },
                    TextRun {
                        len: line.len() - b,
                        ..run
                    },
                ]
                .into_iter()
                .filter(|r| r.len > 0)
                .collect();
            }
        }
        let wrap = if input.multiline { width } else { None };
        let shaped = window
            .text_system()
            .shape_text(
                SharedString::from(line.to_string()),
                font_size,
                &runs,
                wrap,
                None,
            )
            .ok()
            .and_then(|mut v| (!v.is_empty()).then(|| v.remove(0)))
            .unwrap_or_default();
        lines.push(shaped);
        starts.push(start);
        start += line.len() + 1;
    }
    Laid { lines, starts }
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = Vec<PaintQuad>;

    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let input = self.input.clone();
        let multiline = input.read(cx).multiline;
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        if !multiline {
            style.size.height = window.line_height().into();
            return (window.request_layout(style, [], cx), ());
        }
        let line_height = window.line_height();
        let placeholder = Theme::get(cx).text_3;
        let layout = window.request_measured_layout(style, move |known, available, window, cx| {
            let width = known.width.or(match available.width {
                gpui::AvailableSpace::Definite(w) => Some(w),
                _ => None,
            });
            let laid = shape(input.read(cx), width, window, placeholder);
            let height: Pixels = laid
                .lines
                .iter()
                .map(|l| l.size(line_height).height.max(line_height))
                .fold(px(0.0), |a, b| a + b);
            gpui::size(width.unwrap_or(px(100.0)), height.max(line_height))
        });
        (layout, ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Vec<PaintQuad> {
        let theme = Theme::get(cx).clone();
        let line_height = window.line_height();
        let laid = shape(
            self.input.read(cx),
            Some(bounds.size.width),
            window,
            theme.text_3,
        );
        self.input.update(cx, |input, _| {
            input.lines = laid.lines;
            input.starts = laid.starts;
            input.bounds = Some(bounds);
            input.line_height = line_height;
        });
        let input = self.input.read(cx);
        let mut quads = vec![];
        if input.content.is_empty() {
            if input.focus.is_focused(window) {
                quads.push(gpui::fill(
                    Bounds::new(bounds.origin, gpui::size(px(1.5), line_height)),
                    theme.accent,
                ));
            }
            return quads;
        }
        if !input.selected.is_empty() {
            // One rectangle per visual row the selection covers.
            let (a, b) = (input.selected.start, input.selected.end);
            if let (Some(pa), Some(pb)) = (input.position_of(a), input.position_of(b)) {
                let mut y = pa.y;
                while y <= pb.y + px(0.5) {
                    let left = if y == pa.y { pa.x } else { px(0.0) };
                    let right = if (y - pb.y).abs() < px(0.5) {
                        pb.x
                    } else {
                        bounds.size.width
                    };
                    if right > left {
                        quads.push(gpui::fill(
                            Bounds::from_corners(
                                bounds.origin + point(left, y),
                                bounds.origin + point(right, y + line_height),
                            ),
                            theme.selection_text,
                        ));
                    }
                    y += line_height;
                }
            }
        } else if input.focus.is_focused(window) {
            if let Some(p) = input.position_of(input.cursor()) {
                quads.push(gpui::fill(
                    Bounds::new(bounds.origin + p, gpui::size(px(1.5), line_height)),
                    theme.accent,
                ));
            }
        }
        quads
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        quads: &mut Vec<PaintQuad>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus = self.input.read(cx).focus.clone();
        window.handle_input(
            &focus,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        let caret = if self.input.read(cx).selected.is_empty() {
            quads.pop()
        } else {
            None
        };
        for quad in quads.drain(..) {
            window.paint_quad(quad);
        }
        let line_height = window.line_height();
        let input = self.input.read(cx);
        let mut y = bounds.origin.y;
        let lines = input.lines.clone();
        for line in &lines {
            let _ = line.paint(
                point(bounds.origin.x, y),
                line_height,
                gpui::TextAlign::Left,
                Some(bounds),
                window,
                cx,
            );
            y += line.size(line_height).height.max(line_height);
        }
        if let Some(caret) = caret {
            window.paint_quad(caret);
        }
    }
}

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focused = self.focus.is_focused(window);
        if self.was_focused && !focused {
            self.selecting = false;
            cx.emit(InputEvent::Blur);
        }
        self.was_focused = focused;
        let mono = self.mono;
        div()
            .id("text-input")
            .key_context("TextInput")
            .track_focus(&self.focus)
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::delete_word_left))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::word_left))
            .on_action(cx.listener(Self::word_right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::select_down))
            .on_action(cx.listener(Self::select_word_left))
            .on_action(cx.listener(Self::select_word_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::select_home))
            .on_action(cx.listener(Self::select_end))
            .on_action(cx.listener(Self::enter))
            .on_action(cx.listener(Self::newline))
            .on_action(cx.listener(Self::escape))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::show_character_palette))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::mouse_up))
            .on_mouse_move(cx.listener(Self::mouse_move))
            .w_full()
            .when(mono, |d| d.font_family(crate::ui::theme::FONT_MONO))
            .when(!mono, |d| d.font_family(FONT_UI))
            .child(TextElement { input: cx.entity() })
    }
}

/// A text field in the theme's well: the usual frame around a [`TextInput`].
pub fn field(input: &Entity<TextInput>, focused: bool, cx: &App) -> gpui::Div {
    let theme = Theme::get(cx);
    div()
        .px(px(8.0))
        .py(px(5.0))
        .rounded(px(radius::SM))
        .bg(theme.well)
        .border_1()
        .border_color(if focused {
            theme.accent_ring
        } else {
            theme.line
        })
        .text_size(px(size::BASE))
        .text_color(theme.text)
        .child(input.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[gpui::test]
    fn typing_editing_and_events(cx: &mut gpui::TestAppContext) {
        let input = cx.new(|cx| TextInput::new(cx).multiline());
        let events = std::rc::Rc::new(std::cell::RefCell::new(vec![]));
        let seen = events.clone();
        cx.update(|cx| {
            cx.subscribe(&input, move |_, e: &InputEvent, _| {
                seen.borrow_mut().push(e.clone())
            })
            .detach()
        });
        cx.update(|cx| {
            input.update(cx, |i, cx| {
                i.edit(0..0, "Hello world", cx);
                assert_eq!(i.previous_word(11), 6);
                assert_eq!(i.next_word(0), 5);
                i.edit(5..5, "\nthere", cx);
                assert_eq!(i.text(), "Hello\nthere world");
                assert_eq!(i.line_start(8), 6);
                assert_eq!(i.line_end(8), 17);
                assert_eq!(i.utf8_to_utf16(3), 3);
            })
        });
        assert_eq!(events.borrow().len(), 2);
        let single = cx.new(TextInput::new);
        cx.update(|cx| {
            single.update(cx, |i, cx| {
                i.edit(0..0, "a\nb", cx);
                assert_eq!(i.text(), "a b", "one-line fields fold new lines");
            })
        });
    }
}
