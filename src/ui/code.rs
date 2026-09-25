use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use crate::highlight::{self, TokenKind};
use crate::ui::UiState;
use crate::ui::theme::{self, BG, CURRENT, DIM, FG, MAGENTA};
use crate::view::{ViewModel, code_footer};

pub fn render(frame: &mut Frame, area: Rect, model: &ViewModel, ui: &mut UiState) {
    let block = theme::pane("Code");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 || inner.width == 0 {
        return;
    }

    let view = inner.height as usize;
    let total = model.source_lines.len();
    if !ui.code_stick
        && let Some(line) = model.line
    {
        let index = line.saturating_sub(1) as usize;
        ui.code_scroll = index.saturating_sub(view / 2);
    }
    let max_scroll = total.saturating_sub(view);
    ui.code_scroll = ui.code_scroll.min(max_scroll);
    ui.code_view = view;
    ui.code_len = total;

    if let Some(error) = &model.source_error {
        line_at(frame, inner, 0, Span::styled(error.clone(), theme::dim()));
    } else if model.source_lines.is_empty() {
        let text = code_footer(model);
        line_at(frame, inner, 0, Span::styled(text, theme::dim()));
    } else {
        let mut in_block = false;
        for line in model.source_lines.iter().take(ui.code_scroll) {
            let _ = highlight::highlight_line(line, &mut in_block);
        }
        for row in 0..view {
            let index = ui.code_scroll + row;
            let Some(source) = model.source_lines.get(index) else {
                break;
            };
            let line_no = (index + 1) as i64;
            let current = model.phase == crate::debugger::Phase::Stopped
                && model.line == Some(line_no)
                && model.has_source;
            let breakpoint = model.breakpoint_lines.contains(&line_no);
            paint_source_line(
                frame,
                Rect::new(inner.x, inner.y + row as u16, inner.width, 1),
                line_no,
                source,
                current,
                breakpoint,
                &mut in_block,
            );
        }
    }
}

fn paint_source_line(
    frame: &mut Frame,
    area: Rect,
    line_no: i64,
    source: &str,
    current: bool,
    breakpoint: bool,
    in_block: &mut bool,
) {
    let bg = if current { CURRENT } else { BG };
    frame.buffer_mut().set_style(area, Style::new().bg(bg));

    let number_style = Style::new()
        .fg(if breakpoint { MAGENTA } else { DIM })
        .bg(bg);
    let arrow = if current { "→" } else { " " };
    let arrow_style = Style::new().fg(MAGENTA).bg(bg);
    let mut spans = vec![
        Span::styled(format!("{line_no:>4} "), number_style),
        Span::styled(arrow, arrow_style),
        Span::styled(" ", Style::new().bg(bg)),
    ];
    for token in highlight::highlight_line(source, in_block) {
        spans.push(Span::styled(token.text, token_style(token.kind, bg)));
    }
    Line::from(spans).render(area, frame.buffer_mut());
}

fn token_style(kind: TokenKind, bg: ratatui::style::Color) -> Style {
    let fg = match kind {
        TokenKind::Plain => FG,
        TokenKind::Keyword | TokenKind::Preprocessor => MAGENTA,
        TokenKind::Number => theme::YELLOW,
        TokenKind::String => theme::GREEN,
        TokenKind::Comment => DIM,
    };
    Style::new().fg(fg).bg(bg)
}

fn line_at(frame: &mut Frame, inner: Rect, row: usize, span: Span) {
    if row as u16 >= inner.height {
        return;
    }
    Line::from(span).render(
        Rect::new(inner.x, inner.y + row as u16, inner.width, 1),
        frame.buffer_mut(),
    );
}
