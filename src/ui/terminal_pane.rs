use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use crate::ui::ListWindow;
use crate::ui::theme;
use crate::view::{TermStyle, ViewModel};

pub fn render(frame: &mut Frame, area: Rect, model: &ViewModel, window: &mut ListWindow) {
    let block = theme::pane("Terminal");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    window.stick_to_end = true;
    let start = window.visible_start(model.terminal.len(), inner.height as usize);
    for (row, line) in model.terminal.iter().enumerate().skip(start) {
        let visible = row - start;
        if visible >= inner.height as usize {
            break;
        }
        let style = match line.style {
            TermStyle::Out | TermStyle::Echo => theme::base(),
            TermStyle::Err | TermStyle::Error => Style::new().fg(theme::RED).bg(theme::BG),
        };
        Line::from(Span::styled(line.text.clone(), style)).render(
            Rect::new(inner.x, inner.y + visible as u16, inner.width, 1),
            frame.buffer_mut(),
        );
    }
}
