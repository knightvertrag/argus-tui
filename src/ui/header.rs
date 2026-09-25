use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Widget};

use crate::ui::theme;
use crate::view::{ViewModel, code_footer};

pub fn render(frame: &mut Frame, area: Rect, model: &ViewModel) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let name = model.file_name.as_deref().unwrap_or("no file");
    let block = Block::bordered()
        .border_style(theme::border())
        .style(theme::base())
        .title(Span::styled(format!(" {name} "), theme::title()));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    Paragraph::new(Line::from(Span::styled(code_footer(model), theme::dim())))
        .render(inner, frame.buffer_mut());
}
