use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Widget};

use crate::ui::Tab;
use crate::ui::theme::{self, BG, MAGENTA};

const LABELS: [(Tab, &str); 6] = [
    (Tab::Code, "[1] Code"),
    (Tab::Vars, "Vars"),
    (Tab::Stack, "Stack"),
    (Tab::Threads, "Threads"),
    (Tab::Memory, "Memory"),
    (Tab::Terminal, "Terminal"),
];

pub fn render(frame: &mut Frame, area: Rect, active: Tab) {
    if area.width == 0 || area.height < 3 {
        return;
    }
    frame.buffer_mut().set_style(area, Style::new().bg(BG));

    let mut x = area.x;
    for (tab, label) in LABELS {
        let text_width = label.chars().count() as u16;
        if tab == active {
            let width = text_width.saturating_add(2);
            if x.saturating_add(width) > area.x.saturating_add(area.width) {
                break;
            }
            let rect = Rect::new(x, area.y, width, 3);
            let block = Block::bordered()
                .border_style(Style::new().fg(MAGENTA))
                .style(theme::base());
            let inner = block.inner(rect);
            frame.render_widget(block, rect);
            Line::from(Span::styled(label, theme::magenta())).render(inner, frame.buffer_mut());
            x = x.saturating_add(width).saturating_add(1);
        } else {
            let width = text_width.saturating_add(2);
            if x.saturating_add(width) > area.x.saturating_add(area.width) {
                break;
            }
            Line::from(Span::styled(format!(" {label} "), theme::dim()))
                .render(Rect::new(x, area.y + 1, width, 1), frame.buffer_mut());
            x = x.saturating_add(width);
        }
    }
}
