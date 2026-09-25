use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use crate::ui::ListWindow;
use crate::ui::theme;
use crate::view::ViewModel;

pub fn render(frame: &mut Frame, area: Rect, model: &ViewModel, window: Option<&mut ListWindow>) {
    let block = theme::pane("Variables");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let start = match window {
        Some(window) => window.visible_start(model.locals.len(), inner.height as usize),
        None => 0,
    };
    for (row, local) in model.locals.iter().enumerate().skip(start) {
        let visible = row - start;
        if visible >= inner.height as usize {
            break;
        }
        let width = inner.width as usize;
        let value = fit(
            &local.value,
            width.saturating_sub(local.head.chars().count()).max(1),
        );
        let head = fit(&local.head, width.saturating_sub(value.chars().count()));
        Line::from(vec![
            Span::styled(head, theme::base()),
            Span::styled(value, theme::yellow()),
        ])
        .render(
            Rect::new(inner.x, inner.y + visible as u16, inner.width, 1),
            frame.buffer_mut(),
        );
    }
}

fn fit(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        text.to_string()
    } else {
        text.chars().take(width).collect()
    }
}
