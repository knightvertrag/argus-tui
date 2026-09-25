use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use crate::ui::theme;
use crate::view::ViewModel;

pub fn render(frame: &mut Frame, area: Rect, model: &ViewModel) {
    let block = theme::pane("Watch");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    for (index, watch) in model.watches.iter().enumerate() {
        if index as u16 >= inner.height {
            break;
        }
        let y = inner.y + index as u16;
        let value = if watch.value.is_empty() {
            String::new()
        } else {
            format!(" {}", watch.value)
        };
        let value_width = value.chars().count().min(inner.width as usize);
        let name_width = (inner.width as usize).saturating_sub(value_width);
        let name = fit(&watch.expression, name_width);
        Line::from(vec![
            Span::styled(name, theme::base()),
            Span::styled(fit(&value, value_width), theme::green()),
        ])
        .render(Rect::new(inner.x, y, inner.width, 1), frame.buffer_mut());
    }
}

fn fit(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        text.to_string()
    } else {
        text.chars().take(width).collect()
    }
}
