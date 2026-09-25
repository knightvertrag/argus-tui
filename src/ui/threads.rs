use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use crate::ui::ListWindow;
use crate::ui::theme;
use crate::view::ViewModel;

pub fn render(frame: &mut Frame, area: Rect, model: &ViewModel, window: &mut ListWindow) {
    let block = theme::pane("Threads");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let start = window.visible_start(model.threads.len(), inner.height as usize);
    for (row, thread) in model.threads.iter().enumerate().skip(start) {
        let visible = row - start;
        if visible >= inner.height as usize {
            break;
        }
        let marker = if thread.focused { "→" } else { " " };
        let marker_style = if thread.focused {
            theme::cyan()
        } else {
            theme::base()
        };
        let text = format!(" {marker} {}  {}", thread.id, thread.name);
        Line::from(Span::styled(text, marker_style)).render(
            Rect::new(inner.x, inner.y + visible as u16, inner.width, 1),
            frame.buffer_mut(),
        );
    }
}
