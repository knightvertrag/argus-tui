use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Widget};

use crate::ui::theme::{self, BG, CYAN, FG};
use crate::ui::{Focus, UiState};
use crate::view::ViewModel;

pub fn render(frame: &mut Frame, area: Rect, model: &ViewModel, ui: &UiState) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let border = if ui.focus == Focus::Prompt {
        theme::magenta()
    } else {
        theme::border()
    };
    let block = Block::bordered()
        .border_style(border)
        .style(theme::base())
        .title(Span::styled(" Command ", theme::title()));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let help = "Press ? for help";
    let help_width = help.chars().count() as u16;
    let show_help = inner.width > help_width + 8
        && ui.message.is_none()
        && (model.status.is_empty() || ui.focus == Focus::Prompt);
    if show_help {
        Line::from(Span::styled(help, theme::dim())).render(
            Rect::new(inner.x + inner.width - help_width, inner.y, help_width, 1),
            frame.buffer_mut(),
        );
    }

    let usable = if show_help {
        inner.width.saturating_sub(help_width).saturating_sub(1)
    } else {
        inner.width
    };
    let mut spans = vec![Span::styled("> ", Style::new().fg(FG).bg(BG))];
    match ui.focus {
        Focus::Prompt => {
            let (before, cursor, after) = split_cursor(&ui.prompt, ui.cursor);
            spans.push(Span::styled(before, Style::new().fg(FG).bg(BG)));
            let cursor_text = if cursor.is_empty() {
                " ".to_string()
            } else {
                cursor
            };
            spans.push(Span::styled(cursor_text, Style::new().fg(BG).bg(CYAN)));
            spans.push(Span::styled(after, Style::new().fg(FG).bg(BG)));
        }
        Focus::View => {
            let message = ui
                .message
                .as_deref()
                .filter(|text| !text.is_empty())
                .unwrap_or(&model.status);
            if !message.is_empty() {
                spans.push(Span::styled(message.to_string(), theme::dim()));
            }
        }
    }
    Paragraph::new(Line::from(spans))
        .render(Rect::new(inner.x, inner.y, usable, 1), frame.buffer_mut());
}

fn split_cursor(text: &str, cursor: usize) -> (String, String, String) {
    let cursor = floor_boundary(text, cursor.min(text.len()));
    let before = text[..cursor].to_string();
    if cursor >= text.len() {
        return (before, String::new(), String::new());
    }
    let ch = text[cursor..].chars().next().unwrap();
    let next = cursor + ch.len_utf8();
    (before, ch.to_string(), text[next..].to_string())
}

pub fn insert(ui: &mut UiState, ch: char) {
    let cursor = floor_boundary(&ui.prompt, ui.cursor.min(ui.prompt.len()));
    ui.prompt.insert(cursor, ch);
    ui.cursor = cursor + ch.len_utf8();
}

pub fn backspace(ui: &mut UiState) {
    let cursor = floor_boundary(&ui.prompt, ui.cursor.min(ui.prompt.len()));
    if cursor == 0 {
        return;
    }
    let prev = prev_boundary(&ui.prompt, cursor);
    ui.prompt.replace_range(prev..cursor, "");
    ui.cursor = prev;
}

pub fn clear(ui: &mut UiState) {
    ui.prompt.clear();
    ui.cursor = 0;
}

pub fn move_left(ui: &mut UiState) {
    ui.cursor = prev_boundary(
        &ui.prompt,
        floor_boundary(&ui.prompt, ui.cursor.min(ui.prompt.len())),
    );
}

pub fn move_right(ui: &mut UiState) {
    let cursor = floor_boundary(&ui.prompt, ui.cursor.min(ui.prompt.len()));
    ui.cursor = next_boundary(&ui.prompt, cursor);
}

fn floor_boundary(text: &str, mut index: usize) -> usize {
    if index > text.len() {
        index = text.len();
    }
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

fn prev_boundary(text: &str, index: usize) -> usize {
    if index == 0 {
        return 0;
    }
    text[..index]
        .char_indices()
        .next_back()
        .map(|(i, _)| i)
        .unwrap_or(0)
}

fn next_boundary(text: &str, index: usize) -> usize {
    if index >= text.len() {
        return text.len();
    }
    let ch = text[index..].chars().next().unwrap();
    index + ch.len_utf8()
}
