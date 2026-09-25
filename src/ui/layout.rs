use ratatui::layout::{Constraint, Layout, Rect};

pub struct Chrome {
    pub header: Rect,
    pub body: Rect,
    pub tabs: Rect,
    pub prompt: Rect,
}

pub fn chrome(area: Rect) -> Chrome {
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(1),
        Constraint::Length(3),
        Constraint::Length(3),
    ])
    .split(area);
    Chrome {
        header: chunks[0],
        body: chunks[1],
        tabs: chunks[2],
        prompt: chunks[3],
    }
}

/// Dense Code tab: file context on the left, source in the middle, values on the right.
pub struct CodeView {
    pub breakpoints: Rect,
    pub watch: Rect,
    pub stack: Rect,
    pub code: Rect,
    pub variables: Rect,
    pub registers: Rect,
}

pub fn code_view(area: Rect) -> CodeView {
    let columns = Layout::horizontal([
        Constraint::Percentage(24),
        Constraint::Percentage(50),
        Constraint::Percentage(26),
    ])
    .split(area);
    let left = Layout::vertical([
        Constraint::Length(7),
        Constraint::Length(6),
        Constraint::Min(3),
    ])
    .split(columns[0]);
    let right = Layout::vertical([Constraint::Min(3), Constraint::Length(8)]).split(columns[2]);
    CodeView {
        breakpoints: left[0],
        watch: left[1],
        stack: left[2],
        code: columns[1],
        variables: right[0],
        registers: right[1],
    }
}
