use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use crate::ui::theme;

pub fn render(frame: &mut Frame, area: Rect) {
    let block = theme::pane("Memory");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    Line::from(Span::styled("(not available)", theme::dim())).render(inner, frame.buffer_mut());
}
