//! The Markdown agents write in replies, drawn with GPUI text: paragraphs, headings, lists
//! (nested, numbered, task boxes), quotes, rules, fenced code, GFM tables and, inline,
//! **bold**, *italic*, `code`, ~~strike~~ and links. Raw HTML is dropped and images show
//! their alt text, so a reply never runs markup or fetches anything; a link opens in the
//! browser only when clicked. Streaming text is parsed as it is: an unclosed marker stays
//! literal until its closing half arrives.

use crate::ui::theme::{radius, size, Theme, FONT_MONO, FONT_UI};
use gpui::{
    div, prelude::*, px, AnyElement, App, ElementId, FontStyle, FontWeight, InteractiveText,
    SharedString, StrikethroughStyle, StyledText, TextRun, UnderlineStyle,
};
use std::ops::Range;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
    pub strike: bool,
    /// Index into [`Inline::links`].
    pub link: Option<usize>,
}

/// One run of text with its styled ranges.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Inline {
    pub text: String,
    pub spans: Vec<(Range<usize>, Style)>,
    pub links: Vec<String>,
}

impl Inline {
    fn push(&mut self, text: &str, style: Style) {
        if text.is_empty() {
            return;
        }
        let start = self.text.len();
        self.text.push_str(text);
        let end = self.text.len();
        match self.spans.last_mut() {
            Some((range, last)) if *last == style && range.end == start => range.end = end,
            _ => self.spans.push((start..end, style)),
        }
    }
    /// The ranges that are links, with their address, for click handling.
    pub fn link_ranges(&self) -> Vec<(Range<usize>, String)> {
        let mut out: Vec<(Range<usize>, String)> = vec![];
        for (range, style) in &self.spans {
            if let Some(i) = style.link {
                match out.last_mut() {
                    Some((last, url)) if last.end == range.start && *url == self.links[i] => {
                        last.end = range.end
                    }
                    _ => out.push((range.clone(), self.links[i].clone())),
                }
            }
        }
        out
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub depth: usize,
    /// The number of an ordered item.
    pub number: Option<u64>,
    /// A task box: `- [ ]` or `- [x]`.
    pub checked: Option<bool>,
    pub inline: Inline,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Block {
    Heading(u8, Inline),
    Paragraph(Inline),
    List(Vec<Item>),
    Code(String),
    Quote(Vec<Block>),
    Rule,
    Table {
        header: Vec<Inline>,
        rows: Vec<Vec<Inline>>,
    },
}

// ---------------------------------------------------------------------------------------
// Inline parsing.

/// The closing `delimiter` for one opened at `from`, with something between and no space
/// just inside it.
fn closing(s: &str, from: usize, delimiter: &str) -> Option<usize> {
    let mut at = from;
    while let Some(found) = s[at..].find(delimiter) {
        let mut index = at + found;
        if delimiter.len() == 2 {
            // `***` after `*it*` inside bold: the bold closes on the run's last two.
            while s[index + 2..].starts_with(&delimiter[..1]) {
                index += 1;
            }
        }
        let before = s[..index].chars().next_back();
        // `**` must not close a single `*` and vice versa.
        let doubled = delimiter.len() == 1
            && (s[index + 1..].starts_with(delimiter) || s[..index].ends_with(delimiter));
        if index > from && before.is_some_and(|c| !c.is_whitespace()) && !doubled {
            return Some(index);
        }
        at = index + delimiter.len();
        if doubled {
            // Skip the whole run.
            while s[at..].starts_with(delimiter) {
                at += delimiter.len();
            }
        }
    }
    None
}

/// `[text](url)` at `i` (on the `[`): (text, url, end).
fn link_at(s: &str, i: usize) -> Option<(&str, &str, usize)> {
    let mut depth = 0;
    let mut close = None;
    for (offset, c) in s[i..].char_indices() {
        match c {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(i + offset);
                    break;
                }
            }
            _ => {}
        }
    }
    let close = close?;
    let rest = &s[close + 1..];
    if !rest.starts_with('(') {
        return None;
    }
    let end = rest.find(')')?;
    let target = rest[1..end].trim();
    // `[a](url "title")`: the address is the first word.
    let url = target.split_whitespace().next().unwrap_or("");
    Some((&s[i + 1..close], url, close + 1 + end + 1))
}

fn is_url_start(s: &str) -> bool {
    s.starts_with("https://") || s.starts_with("http://")
}

/// Only web addresses become clickable links.
fn safe_url(url: &str) -> Option<String> {
    is_url_start(url).then(|| url.to_string())
}

fn parse_into(s: &str, style: Style, out: &mut Inline) {
    let mut plain_start = 0;
    let mut i = 0;
    let flush = |out: &mut Inline, from: usize, to: usize| out.push(&s[from..to], style);
    while i < s.len() {
        let rest = &s[i..];
        let c = rest.chars().next().unwrap_or(' ');
        let prev = s[..i].chars().next_back();
        // Escapes.
        if c == '\\' {
            if let Some(next) = rest[1..]
                .chars()
                .next()
                .filter(|n| n.is_ascii_punctuation())
            {
                flush(out, plain_start, i);
                out.push(&next.to_string(), style);
                i += 1 + next.len_utf8();
                plain_start = i;
                continue;
            }
        }
        // Code spans.
        if c == '`' {
            let ticks = rest.len() - rest.trim_start_matches('`').len();
            let fence = &rest[..ticks];
            if let Some(end) = rest[ticks..].find(fence) {
                flush(out, plain_start, i);
                let inner = &rest[ticks..ticks + end];
                let inner = if inner.len() > 2 && inner.starts_with(' ') && inner.ends_with(' ') {
                    &inner[1..inner.len() - 1]
                } else {
                    inner
                };
                out.push(
                    inner,
                    Style {
                        code: true,
                        ..style
                    },
                );
                i += ticks * 2 + end;
                plain_start = i;
                continue;
            }
            i += ticks;
            continue;
        }
        // Images show their alt text; nothing is fetched.
        if rest.starts_with("![") {
            if let Some((alt, _, end)) = link_at(s, i + 1) {
                flush(out, plain_start, i);
                parse_into(alt, style, out);
                i = end;
                plain_start = i;
                continue;
            }
        }
        if c == '[' && style.link.is_none() {
            if let Some((text, url, end)) = link_at(s, i) {
                flush(out, plain_start, i);
                let link = safe_url(url).map(|url| {
                    out.links.push(url);
                    out.links.len() - 1
                });
                parse_into(text, Style { link, ..style }, out);
                i = end;
                plain_start = i;
                continue;
            }
        }
        // Raw HTML is dropped; `<https://…>` is a link.
        if c == '<' {
            let next = rest[1..].chars().next();
            if let Some(close) = rest.find('>') {
                let inside = &rest[1..close];
                if is_url_start(inside) && !inside.contains(char::is_whitespace) {
                    flush(out, plain_start, i);
                    out.links.push(inside.to_string());
                    let link = Some(out.links.len() - 1);
                    out.push(inside, Style { link, ..style });
                    i += close + 1;
                    plain_start = i;
                    continue;
                }
                if next.is_some_and(|n| n.is_ascii_alphabetic() || n == '/' || n == '!') {
                    flush(out, plain_start, i);
                    i += close + 1;
                    plain_start = i;
                    continue;
                }
            }
        }
        // Bare web addresses.
        if style.link.is_none()
            && is_url_start(rest)
            && prev.is_none_or(|p| p.is_whitespace() || p == '(')
        {
            let len = rest
                .find(|ch: char| ch.is_whitespace() || ch == '<' || ch == ')')
                .unwrap_or(rest.len());
            let url = rest[..len].trim_end_matches(['.', ',', ';', ':', '!', '?']);
            flush(out, plain_start, i);
            out.links.push(url.to_string());
            let link = Some(out.links.len() - 1);
            out.push(url, Style { link, ..style });
            i += url.len();
            plain_start = i;
            continue;
        }
        // Emphasis: `**`/`__` bold, `*`/`_` italic, `~~` strike.
        let mut consumed = false;
        for (delimiter, set) in [("**", 0u8), ("__", 0), ("~~", 2), ("*", 1), ("_", 1)] {
            if !rest.starts_with(delimiter) {
                continue;
            }
            let opens = rest[delimiter.len()..]
                .chars()
                .next()
                .is_some_and(|n| !n.is_whitespace());
            // `snake_case` is not emphasis.
            let intraword = delimiter.starts_with('_') && prev.is_some_and(char::is_alphanumeric);
            if !opens || intraword {
                continue;
            }
            let from = i + delimiter.len();
            let Some(end) = closing(s, from, delimiter) else {
                continue;
            };
            if delimiter.starts_with('_')
                && s[end + delimiter.len()..]
                    .chars()
                    .next()
                    .is_some_and(char::is_alphanumeric)
            {
                continue;
            }
            flush(out, plain_start, i);
            let inner = match set {
                0 => Style {
                    bold: true,
                    ..style
                },
                1 => Style {
                    italic: true,
                    ..style
                },
                _ => Style {
                    strike: true,
                    ..style
                },
            };
            parse_into(&s[from..end], inner, out);
            i = end + delimiter.len();
            plain_start = i;
            consumed = true;
            break;
        }
        if !consumed {
            // Plain text, or a marker that opened nothing: it stays as text.
            i += c.len_utf8();
        }
    }
    flush(out, plain_start.min(s.len()), s.len());
}

/// Inline Markdown into text and styled ranges.
pub fn inline(s: &str) -> Inline {
    let mut out = Inline::default();
    parse_into(s, Style::default(), &mut out);
    out
}

// ---------------------------------------------------------------------------------------
// Block parsing.

fn heading(line: &str) -> Option<(u8, &str)> {
    let hashes = line.len() - line.trim_start_matches('#').len();
    if (1..=6).contains(&hashes) {
        let rest = &line[hashes..];
        if rest.is_empty() || rest.starts_with(' ') {
            return Some((hashes as u8, rest.trim().trim_end_matches('#').trim()));
        }
    }
    None
}

fn is_rule(line: &str) -> bool {
    let t: String = line.chars().filter(|c| !c.is_whitespace()).collect();
    t.len() >= 3
        && ["*", "-", "_"]
            .iter()
            .any(|m| t.chars().all(|c| c.to_string() == *m))
}

fn fence(line: &str) -> Option<&str> {
    let t = line.trim_start();
    ["```", "~~~"].into_iter().find(|f| t.starts_with(f))
}

/// `- item`, `* item`, `1. item`: (indent, number, text).
fn list_item(line: &str) -> Option<(usize, Option<u64>, &str)> {
    let indent = line.len() - line.trim_start().len();
    let t = line.trim_start();
    for bullet in ["- ", "* ", "+ "] {
        if let Some(rest) = t.strip_prefix(bullet) {
            return Some((indent, None, rest));
        }
    }
    let digits = t.len() - t.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    if (1..=9).contains(&digits) {
        let rest = &t[digits..];
        if rest.starts_with(". ") || rest.starts_with(") ") {
            return Some((indent, t[..digits].parse().ok(), &rest[2..]));
        }
    }
    None
}

/// The opening tag of an HTML block line: `<div …`, `</p>`, `<!-- …`.
fn html_tag(line: &str) -> Option<String> {
    let t = line.trim_start().strip_prefix('<')?;
    let t = t.strip_prefix('/').unwrap_or(t);
    if t.starts_with('!') {
        return Some("!".into());
    }
    let name: String = t
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect();
    (!name.is_empty() && name.chars().next().is_some_and(|c| c.is_ascii_alphabetic()))
        .then(|| name.to_lowercase())
}

fn table_cells(line: &str) -> Vec<&str> {
    let t = line.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t.strip_suffix('|').unwrap_or(t);
    t.split('|').map(str::trim).collect()
}

fn is_table_separator(line: &str) -> bool {
    line.contains('-')
        && table_cells(line)
            .iter()
            .all(|c| !c.is_empty() && c.chars().all(|ch| matches!(ch, '-' | ':' | ' ')))
}

/// Markdown into blocks.
pub fn parse(text: &str) -> Vec<Block> {
    let lines: Vec<&str> = text.lines().collect();
    let mut blocks = vec![];
    let mut paragraph: Vec<&str> = vec![];
    let end_paragraph = |paragraph: &mut Vec<&str>, blocks: &mut Vec<Block>| {
        if paragraph.is_empty() {
            return;
        }
        // A line ending in two spaces is a hard break; otherwise lines join.
        let mut joined = String::new();
        for (n, line) in paragraph.iter().enumerate() {
            if n > 0 {
                joined.push(if paragraph[n - 1].ends_with("  ") {
                    '\n'
                } else {
                    ' '
                });
            }
            joined.push_str(line.trim());
        }
        blocks.push(Block::Paragraph(inline(&joined)));
        paragraph.clear();
    };
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();
        if trimmed.is_empty() {
            end_paragraph(&mut paragraph, &mut blocks);
            i += 1;
            continue;
        }
        if let Some(marker) = fence(line) {
            end_paragraph(&mut paragraph, &mut blocks);
            let mut code = vec![];
            i += 1;
            while i < lines.len() && !lines[i].trim_start().starts_with(marker) {
                code.push(lines[i]);
                i += 1;
            }
            blocks.push(Block::Code(code.join("\n")));
            i += 1;
            continue;
        }
        if let Some(tag) = html_tag(line).filter(|_| paragraph.is_empty()) {
            // An HTML block: skipped whole. Script-like ones run to their closing tag.
            let raw = matches!(tag.as_str(), "script" | "style" | "pre" | "textarea");
            let closing_tag = format!("</{tag}>");
            while i < lines.len() {
                let l = lines[i];
                i += 1;
                if raw {
                    if l.to_lowercase().contains(&closing_tag) {
                        break;
                    }
                } else if i >= lines.len() || lines[i].trim().is_empty() {
                    break;
                }
            }
            continue;
        }
        if let Some((level, text)) = heading(trimmed) {
            end_paragraph(&mut paragraph, &mut blocks);
            blocks.push(Block::Heading(level, inline(text)));
            i += 1;
            continue;
        }
        if is_rule(trimmed) && (paragraph.is_empty() || !trimmed.starts_with('-')) {
            end_paragraph(&mut paragraph, &mut blocks);
            blocks.push(Block::Rule);
            i += 1;
            continue;
        }
        if trimmed.contains('|') && lines.get(i + 1).is_some_and(|l| is_table_separator(l)) {
            end_paragraph(&mut paragraph, &mut blocks);
            let header = table_cells(line).into_iter().map(inline).collect();
            let mut rows = vec![];
            i += 2;
            while i < lines.len() && lines[i].contains('|') && !lines[i].trim().is_empty() {
                rows.push(table_cells(lines[i]).into_iter().map(inline).collect());
                i += 1;
            }
            blocks.push(Block::Table { header, rows });
            continue;
        }
        if trimmed.starts_with('>') {
            end_paragraph(&mut paragraph, &mut blocks);
            let mut quoted = vec![];
            while i < lines.len() && lines[i].trim_start().starts_with('>') {
                let l = lines[i].trim_start()[1..].strip_prefix(' ');
                quoted.push(l.unwrap_or(&lines[i].trim_start()[1..]));
                i += 1;
            }
            blocks.push(Block::Quote(parse(&quoted.join("\n"))));
            continue;
        }
        if list_item(line).is_some() && (paragraph.is_empty() || list_item(line).is_some()) {
            end_paragraph(&mut paragraph, &mut blocks);
            let mut items: Vec<Item> = vec![];
            let mut texts: Vec<String> = vec![];
            let mut indents: Vec<usize> = vec![];
            while i < lines.len() {
                let l = lines[i];
                if let Some((indent, number, text)) = list_item(l) {
                    // Depth follows the distinct indents seen so far.
                    while indents.last().is_some_and(|last| *last > indent) {
                        indents.pop();
                    }
                    if indents.last().is_none_or(|last| *last < indent) {
                        indents.push(indent);
                    }
                    let (checked, text) = if let Some(rest) = text.strip_prefix("[ ] ") {
                        (Some(false), rest)
                    } else if let Some(rest) = text
                        .strip_prefix("[x] ")
                        .or_else(|| text.strip_prefix("[X] "))
                    {
                        (Some(true), rest)
                    } else {
                        (None, text)
                    };
                    items.push(Item {
                        depth: (indents.len() - 1).min(3),
                        number,
                        checked,
                        inline: Inline::default(),
                    });
                    texts.push(text.trim().to_string());
                    i += 1;
                } else if l.trim().is_empty() {
                    // A blank line inside a list continues it when an item follows.
                    if lines
                        .get(i + 1)
                        .is_some_and(|next| list_item(next).is_some())
                    {
                        i += 1;
                    } else {
                        break;
                    }
                } else if l.starts_with([' ', '\t']) && !texts.is_empty() {
                    // A continuation line of the last item.
                    let last = texts.last_mut().expect("an item");
                    last.push(' ');
                    last.push_str(l.trim());
                    i += 1;
                } else {
                    break;
                }
            }
            for (item, text) in items.iter_mut().zip(&texts) {
                item.inline = inline(text);
            }
            blocks.push(Block::List(items));
            continue;
        }
        paragraph.push(line);
        i += 1;
    }
    end_paragraph(&mut paragraph, &mut blocks);
    blocks
}

// ---------------------------------------------------------------------------------------
// Drawing.

/// Text runs for an inline: the interface face, bold, italic, mono for code, accent links.
fn runs(inline: &Inline, base: gpui::Hsla, theme: &Theme) -> Vec<TextRun> {
    let mut runs = vec![];
    let mut at = 0;
    let plain = |len: usize| TextRun {
        len,
        font: gpui::font(FONT_UI),
        color: base,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    for (range, style) in &inline.spans {
        if range.start > at {
            runs.push(plain(range.start - at));
        }
        let mut font = gpui::font(if style.code { FONT_MONO } else { FONT_UI });
        if style.bold {
            font.weight = FontWeight::BOLD;
        }
        if style.italic {
            font.style = FontStyle::Italic;
        }
        let color = if style.link.is_some() {
            theme.accent_text
        } else if style.bold {
            theme.text
        } else {
            base
        };
        runs.push(TextRun {
            len: range.len(),
            font,
            color,
            background_color: style.code.then_some(theme.well),
            underline: style.link.map(|_| UnderlineStyle {
                thickness: px(1.0),
                color: Some(theme.accent_ring),
                wavy: false,
            }),
            strikethrough: style.strike.then_some(StrikethroughStyle {
                thickness: px(1.0),
                color: Some(base),
            }),
        });
        at = range.end;
    }
    if inline.text.len() > at {
        runs.push(plain(inline.text.len() - at));
    }
    runs
}

/// One inline as an element: interactive when it carries links, which open in the browser.
fn text(inline: &Inline, id: ElementId, base: gpui::Hsla, theme: &Theme) -> AnyElement {
    let styled = StyledText::new(SharedString::from(inline.text.clone()))
        .with_runs(runs(inline, base, theme));
    let links = inline.link_ranges();
    if links.is_empty() {
        return styled.into_any_element();
    }
    let ranges = links.iter().map(|(r, _)| r.clone()).collect();
    let urls: Vec<String> = links.into_iter().map(|(_, u)| u).collect();
    InteractiveText::new(id, styled)
        .on_click(ranges, move |index, _, cx| {
            if let Some(url) = urls.get(index) {
                cx.open_url(url);
            }
        })
        .into_any_element()
}

fn render_blocks(blocks: &[Block], key: &str, theme: &Theme, base: gpui::Hsla) -> Vec<AnyElement> {
    blocks
        .iter()
        .enumerate()
        .map(|(n, block)| {
            let id = |sub: usize| ElementId::Name(format!("{key}-{n}-{sub}").into());
            match block {
                Block::Paragraph(inline) => div()
                    .child(text(inline, id(0), base, theme))
                    .into_any_element(),
                Block::Heading(level, inline) => div()
                    .mt(px(6.0))
                    .text_size(px(if *level <= 2 {
                        size::MD
                    } else {
                        size::BASE + 1.0
                    }))
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.text)
                    .child(text(inline, id(0), theme.text, theme))
                    .into_any_element(),
                Block::List(items) => div()
                    .flex()
                    .flex_col()
                    .gap(px(5.0))
                    .children(items.iter().enumerate().map(|(i, item)| {
                        let marker = match (item.checked, item.number) {
                            (Some(true), _) => "☑".to_string(),
                            (Some(false), _) => "☐".to_string(),
                            (None, Some(n)) => format!("{n}."),
                            (None, None) => if item.depth % 2 == 0 { "•" } else { "◦" }.to_string(),
                        };
                        div()
                            .flex()
                            .gap(px(6.0))
                            .pl(px(4.0 + 16.0 * item.depth as f32))
                            .child(
                                div()
                                    .flex_none()
                                    .min_w(px(14.0))
                                    .text_color(theme.text_3)
                                    .child(marker),
                            )
                            .child(div().flex_1().min_w_0().child(text(
                                &item.inline,
                                id(i),
                                base,
                                theme,
                            )))
                    }))
                    .into_any_element(),
                Block::Code(code) => div()
                    .p(px(10.0))
                    .rounded(px(radius::SM))
                    .bg(theme.well)
                    .border_1()
                    .border_color(theme.hairline)
                    .font_family(FONT_MONO)
                    .text_size(px(size::SM - 0.5))
                    .line_height(px(17.0))
                    .text_color(theme.text_2)
                    .child(SharedString::from(code.clone()))
                    .into_any_element(),
                Block::Quote(inner) => div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .pl(px(12.0))
                    .border_l_2()
                    .border_color(theme.accent)
                    .children(render_blocks(
                        inner,
                        &format!("{key}-{n}q"),
                        theme,
                        theme.text_2,
                    ))
                    .into_any_element(),
                Block::Rule => div()
                    .my(px(4.0))
                    .h(px(1.0))
                    .bg(theme.line)
                    .into_any_element(),
                Block::Table { header, rows } => {
                    let row = |cells: &[Inline], r: usize, head: bool| {
                        div().flex().border_b_1().border_color(theme.line).children(
                            cells.iter().enumerate().map(|(c, cell)| {
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .px(px(6.0))
                                    .py(px(5.0))
                                    .when(head, |d| d.font_weight(FontWeight::BOLD))
                                    .child(text(
                                        cell,
                                        id(1000 * r + c),
                                        if head { theme.text } else { base },
                                        theme,
                                    ))
                            }),
                        )
                    };
                    div()
                        .flex()
                        .flex_col()
                        .text_size(px(size::SM))
                        .child(row(header, 0, true))
                        .children(
                            rows.iter()
                                .enumerate()
                                .map(|(r, cells)| row(cells, r + 1, false)),
                        )
                        .into_any_element()
                }
            }
        })
        .collect()
}

/// A reply as elements, one per block. `key` makes the ids of its links unique.
pub fn render(blocks: &[Block], key: &str, cx: &App) -> gpui::Div {
    let theme = Theme::get(cx);
    div()
        .flex()
        .flex_col()
        .gap(px(9.0))
        .children(render_blocks(blocks, key, theme, theme.text))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn styled(inline: &Inline, style: Style) -> Vec<&str> {
        inline
            .spans
            .iter()
            .filter(|(_, s)| *s == style)
            .map(|(r, _)| &inline.text[r.clone()])
            .collect()
    }

    #[test]
    fn renders_markdown_without_html_or_images() {
        let blocks = parse(
            "**Warm bass**\n\n- Kick\n- Snare\n\n<img src=\"https://tracker.invalid/a\">\n\n![x](https://tracker.invalid/b)\n\n<script>alert(1)</script>",
        );
        assert_eq!(blocks.len(), 3, "{blocks:#?}");
        let Block::Paragraph(first) = &blocks[0] else {
            panic!()
        };
        assert_eq!(first.text, "Warm bass");
        assert_eq!(
            styled(
                first,
                Style {
                    bold: true,
                    ..Style::default()
                }
            ),
            ["Warm bass"]
        );
        let Block::List(items) = &blocks[1] else {
            panic!()
        };
        assert_eq!(items.len(), 2);
        let Block::Paragraph(image) = &blocks[2] else {
            panic!()
        };
        assert_eq!(image.text, "x", "an image shows its alt text");
        assert!(image.links.is_empty(), "and is never a link");
        let all = format!("{blocks:?}");
        assert!(!all.contains("alert") && !all.contains("tracker.invalid/a"));
    }

    #[test]
    fn inline_styles_links_and_literal_markers() {
        let i = inline(
            "Use **bold** and *it* with `code` and ~~gone~~ [docs](https://x.y/z) or https://a.b.",
        );
        assert_eq!(
            i.text,
            "Use bold and it with code and gone docs or https://a.b."
        );
        assert_eq!(
            styled(
                &i,
                Style {
                    italic: true,
                    ..Style::default()
                }
            ),
            ["it"]
        );
        assert_eq!(
            styled(
                &i,
                Style {
                    code: true,
                    ..Style::default()
                }
            ),
            ["code"]
        );
        assert_eq!(
            styled(
                &i,
                Style {
                    strike: true,
                    ..Style::default()
                }
            ),
            ["gone"]
        );
        let links = i.link_ranges();
        assert_eq!(links.len(), 2);
        assert_eq!(&i.text[links[0].0.clone()], "docs");
        assert_eq!(links[1].1, "https://a.b");
        // Unclosed markers and snake_case stay as they are while a reply streams in.
        assert_eq!(inline("**bol").text, "**bol");
        assert_eq!(inline("snake_case_name").text, "snake_case_name");
        assert_eq!(inline("2 * 3 * 4").text, "2 * 3 * 4");
        assert_eq!(inline(r"\*not\*").text, "*not*");
        // Only web links are clickable.
        assert!(inline("[x](javascript:alert(1))").links.is_empty());
        assert_eq!(inline("**bold *both***").text, "bold both");
    }

    #[test]
    fn blocks_headings_code_quotes_tables_and_nested_lists() {
        let blocks = parse(
            "# Title\nSome text\nmore\n\n```rust\nlet a = 1;\n\nlet b = 2;\n```\n> quoted **x**\n---\n| a | b |\n|---|:-:|\n| 1 | 2 |\n\n1. one\n   - nested\n2. two\n- [x] done",
        );
        assert!(matches!(&blocks[0], Block::Heading(1, t) if t.text == "Title"));
        assert!(matches!(&blocks[1], Block::Paragraph(t) if t.text == "Some text more"));
        assert!(matches!(&blocks[2], Block::Code(c) if c == "let a = 1;\n\nlet b = 2;"));
        assert!(matches!(&blocks[3], Block::Quote(inner) if inner.len() == 1));
        assert_eq!(blocks[4], Block::Rule);
        assert!(
            matches!(&blocks[5], Block::Table { header, rows } if header.len() == 2 && rows.len() == 1)
        );
        let Block::List(items) = &blocks[6] else {
            panic!("{blocks:#?}")
        };
        assert_eq!(items.len(), 4);
        assert_eq!(items[0].number, Some(1));
        assert_eq!(items[1].depth, 1);
        assert_eq!(items[3].checked, Some(true));
        // An unfinished fence while streaming is still code.
        assert!(matches!(&parse("```\ncode so far")[0], Block::Code(c) if c == "code so far"));
    }
}
