//! A field for secrets (API keys): what is typed shows as bullets, nothing can be copied out
//! of it, and it never holds a saved key. The saved key stays in the settings file; the form
//! only says that one exists ("Key saved · enter a replacement"), so a key is never echoed.
//!
//! It answers the text-field keys of [`super::text_input`] (its key context is `TextInput`,
//! so single-key shortcuts stay off while typing), but keeps one edit point at the end:
//! type or paste to add, Backspace to remove, ⌘A then type or Backspace to replace it all.
//! It emits the same [`InputEvent`]s as a text field.

use super::text_input::{
    Backspace, Copy, Cut, Delete, DeleteWordLeft, Enter, Escape, InputEvent, Paste, SelectAll,
};
use crate::ui::theme::{radius, size, Theme, FONT_MONO};
use gpui::{
    canvas, div, prelude::*, px, App, Bounds, Context, CursorStyle, ElementInputHandler, Entity,
    EntityInputHandler, EventEmitter, FocusHandle, Focusable, Pixels, Point, SharedString,
    UTF16Selection, Window,
};
use std::ops::Range;

pub struct SecretInput {
    focus: FocusHandle,
    content: String,
    placeholder: SharedString,
    /// ⌘A selected everything: the next edit replaces the whole secret.
    all: bool,
    /// IME composition in progress, in bytes of `content`.
    marked: Option<Range<usize>>,
    was_focused: bool,
}

impl EventEmitter<InputEvent> for SecretInput {}

impl SecretInput {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus: cx.focus_handle(),
            content: String::new(),
            placeholder: SharedString::default(),
            all: false,
            marked: None,
            was_focused: false,
        }
    }
    /// What was typed, for saving. Never shown.
    pub fn text(&self) -> &str {
        &self.content
    }
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }
    /// Forget what was typed (after saving, or when the form changes service).
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        if !self.content.is_empty() || self.marked.is_some() {
            self.content.clear();
            self.marked = None;
            self.all = false;
            cx.emit(InputEvent::Changed);
            cx.notify();
        }
    }
    pub fn set_placeholder(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        let text = text.into();
        if text != self.placeholder {
            self.placeholder = text;
            cx.notify();
        }
    }
    /// What the empty field says, such as whether a key is saved.
    #[cfg(test)]
    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }
    pub fn is_focused(&self, window: &Window) -> bool {
        self.focus.is_focused(window)
    }

    /// Add text at the end, as typing or pasting does.
    pub fn insert(&mut self, text: &str, cx: &mut Context<Self>) {
        // A secret is one line: pasted keys often carry a trailing new line.
        let text: String = text.chars().filter(|c| !c.is_control()).collect();
        if std::mem::take(&mut self.all) {
            self.content.clear();
        }
        if let Some(marked) = self.marked.take() {
            self.content.replace_range(marked, "");
        }
        self.content.push_str(&text);
        cx.emit(InputEvent::Changed);
        cx.notify();
    }
    fn backspace(&mut self, _: &Backspace, _: &mut Window, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.all) {
            self.content.clear();
        } else {
            self.content.pop();
        }
        self.marked = None;
        cx.emit(InputEvent::Changed);
        cx.notify();
    }
    fn delete(&mut self, _: &Delete, w: &mut Window, cx: &mut Context<Self>) {
        self.backspace(&Backspace, w, cx);
    }
    /// Nothing in a secret is a word worth keeping: Option-Backspace clears it.
    fn delete_word(&mut self, _: &DeleteWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.all = false;
        self.clear(cx);
    }
    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.all = !self.content.is_empty();
        cx.notify();
    }
    fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.insert(text.trim(), cx);
        }
    }
    /// Copying or cutting a secret would echo it: both do nothing.
    fn copy(&mut self, _: &Copy, _: &mut Window, _: &mut Context<Self>) {}
    fn cut(&mut self, _: &Cut, _: &mut Window, _: &mut Context<Self>) {}
    fn enter(&mut self, _: &Enter, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(InputEvent::Submit);
    }
    fn escape(&mut self, _: &Escape, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(InputEvent::Cancel);
    }

    fn utf16_len(text: &str) -> usize {
        text.encode_utf16().count()
    }
    /// A UTF-16 offset (what the platform speaks) to a byte offset in `content`.
    fn byte_offset(&self, utf16: usize) -> usize {
        let mut count = 0;
        for (i, c) in self.content.char_indices() {
            if count >= utf16 {
                return i;
            }
            count += c.len_utf16();
        }
        self.content.len()
    }
}

impl EntityInputHandler for SecretInput {
    /// The platform may read text around the caret; a secret answers with nothing.
    fn text_for_range(
        &mut self,
        _: Range<usize>,
        _: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        None
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let end = Self::utf16_len(&self.content);
        Some(UTF16Selection {
            range: if self.all { 0..end } else { end..end },
            reversed: false,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked.as_ref().map(|r| {
            Self::utf16_len(&self.content[..r.start])..Self::utf16_len(&self.content[..r.end])
        })
    }
    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.marked = None;
    }
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(range) = range.filter(|_| !self.all && self.marked.is_none()) {
            let range = self.byte_offset(range.start)..self.byte_offset(range.end);
            self.content.replace_range(range, "");
        }
        self.insert(text, cx);
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        _: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if std::mem::take(&mut self.all) {
            self.content.clear();
        }
        let start = match self.marked.take() {
            Some(marked) => {
                self.content.replace_range(marked.clone(), "");
                marked.start
            }
            None => self.content.len(),
        };
        self.content.insert_str(start, text);
        self.marked = (!text.is_empty()).then(|| start..start + text.len());
        cx.notify();
    }
    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        Some(bounds)
    }
    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        Some(Self::utf16_len(&self.content))
    }
}

impl Focusable for SecretInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for SecretInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx).clone();
        let focused = self.focus.is_focused(window);
        if self.was_focused && !focused {
            self.all = false;
            cx.emit(InputEvent::Blur);
        }
        self.was_focused = focused;
        let count = self.content.chars().count();
        let entity = cx.entity();
        let focus = self.focus.clone();
        div()
            .id("secret-input")
            .key_context("TextInput")
            .track_focus(&self.focus)
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::delete_word))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::enter))
            .on_action(cx.listener(Self::escape))
            .relative()
            .w_full()
            .h(px(18.0))
            .flex()
            .items_center()
            .overflow_hidden()
            .whitespace_nowrap()
            .when(count == 0, |d| {
                d.text_color(theme.text_3).child(self.placeholder.clone())
            })
            .when(count > 0, |d| {
                d.child(
                    div()
                        .font_family(FONT_MONO)
                        .text_size(px(size::SM))
                        .text_color(theme.text)
                        .rounded(px(radius::XS))
                        .when(self.all, |d| d.bg(theme.accent_soft))
                        .child("•".repeat(count.min(64))),
                )
            })
            .when(focused, |d| {
                d.child(
                    div()
                        .flex_none()
                        .w(px(1.5))
                        .h(px(14.0))
                        .ml(px(1.0))
                        .bg(theme.accent),
                )
            })
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, cx| {
                        window.handle_input(
                            &focus,
                            ElementInputHandler::new(bounds, entity.clone()),
                            cx,
                        );
                    },
                )
                .absolute()
                .size_full(),
            )
    }
}

/// A secret field in the theme's well, like [`super::field`].
pub fn secret_field(input: &Entity<SecretInput>, focused: bool, cx: &App) -> gpui::Div {
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
    fn typing_replaces_and_never_reads_back(cx: &mut gpui::TestAppContext) {
        let input = cx.new(SecretInput::new);
        cx.update(|cx| {
            input.update(cx, |i, cx| {
                i.insert("sk-test\n", cx);
                assert_eq!(i.text(), "sk-test", "control characters are dropped");
                i.all = true;
                i.insert("other", cx);
                assert_eq!(i.text(), "other", "after select all, typing replaces");
                i.content.push('é');
                assert_eq!(i.byte_offset(6), "other".len() + 'é'.len_utf8());
                i.clear(cx);
                assert!(i.is_empty());
            })
        });
    }
}
