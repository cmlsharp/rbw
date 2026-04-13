use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span, Text},
    widgets::Paragraph,
};

use crate::{app::hint_line, config::Palette};

use super::{State, bindings::bindings, state::Field};

struct FieldLine {
    field: Field,
    label: Option<String>,
    value: String,
}

struct LayoutInfo {
    lines: Vec<FieldLine>,
    scroll: u16,
    cursor_line: u16,
    cursor_column: u16,
}

fn field_label(field: Field, uri_count: usize) -> String {
    match field {
        Field::Uri(i) if uri_count > 1 => format!("URI {}", i + 1),
        _ => field.label().to_string(),
    }
}

fn displayed_value(create: &State, field: Field) -> String {
    let raw_value = create.field_value(field);
    if field == Field::Password && !create.show_password && !raw_value.is_empty() {
        "•".repeat(raw_value.chars().count())
    } else {
        raw_value.to_string()
    }
}

fn wrap_segments(value: &str, first_width: usize, continuation_width: usize) -> Vec<String> {
    let mut segments = Vec::new();

    for logical_line in value.split('\n') {
        if logical_line.is_empty() {
            segments.push(String::new());
            continue;
        }

        let chars: Vec<char> = logical_line.chars().collect();
        let mut start = 0;
        let mut width = first_width.max(1);
        while start < chars.len() {
            let end = (start + width).min(chars.len());
            segments.push(chars[start..end].iter().collect());
            start = end;
            width = continuation_width.max(1);
        }
    }

    if segments.is_empty() {
        segments.push(String::new());
    }

    segments
}

fn build_layout(create: &State, width: u16, height: u16) -> LayoutInfo {
    let width = width.max(1) as usize;
    let mut lines = Vec::new();
    let mut cursor_line = 0u16;
    let mut cursor_column = 0u16;

    let fields = create.fields();
    let uri_count = create.draft.uris.len();
    let max_label_width = fields
        .iter()
        .map(|f| field_label(*f, uri_count).chars().count())
        .max()
        .unwrap_or(0)
        + 2;

    for &field in &fields {
        let label = field_label(field, uri_count);
        let label_width = max_label_width;
        let first_width = width.saturating_sub(label_width).max(1);
        let continuation_width = first_width;
        let displayed_value = displayed_value(create, field);
        let segments = wrap_segments(&displayed_value, first_width, continuation_width);
        let field_start = lines.len() as u16;

        for (index, segment) in segments.iter().enumerate() {
            lines.push(FieldLine {
                field,
                label: if index == 0 { Some(label.clone()) } else { None },
                value: segment.clone(),
            });
        }

        if field == create.field {
            let value_len = displayed_value.chars().count();
            if value_len <= first_width {
                cursor_line = field_start;
                cursor_column = (label_width + value_len) as u16;
            } else {
                let remaining = value_len - first_width;
                let extra_rows = remaining / continuation_width;
                let extra_cols = remaining % continuation_width;
                cursor_line = field_start + extra_rows as u16 + 1;
                cursor_column = (label_width + extra_cols) as u16;
                if extra_cols == 0 {
                    cursor_line = cursor_line.saturating_sub(1);
                    cursor_column = (label_width + continuation_width) as u16;
                }
            }
        }
    }

    let height = height.max(1);
    let scroll = cursor_line.saturating_sub(height.saturating_sub(1));

    LayoutInfo {
        lines,
        scroll,
        cursor_line,
        cursor_column,
    }
}

pub(crate) fn render_modal(frame: &mut ratatui::Frame<'_>, palette: &Palette, create: &State) {
    let inner =
        crate::app::render_popup_shell(frame, 76, 18, create.title(), palette.accent);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(2)])
        .split(inner);
    let layout = build_layout(create, rows[0].width, rows[0].height);
    let uri_count = create.draft.uris.len();
    let max_label_width = create
        .fields()
        .iter()
        .map(|f| field_label(*f, uri_count).chars().count())
        .max()
        .unwrap_or(0)
        + 2;
    let lines = layout
        .lines
        .into_iter()
        .map(|line| {
            let value_style = if line.field == create.field && create.replace_on_input {
                Style::default()
                    .fg(palette.selected_fg)
                    .bg(palette.selected_bg)
            } else {
                Style::default().fg(palette.text)
            };
            let mut spans = Vec::new();
            if let Some(label) = line.label {
                let padded = format!("{:>width$}: ", label, width = max_label_width - 2);
                spans.push(Span::styled(padded, Style::default().fg(palette.accent)));
            } else {
                spans.push(Span::raw(" ".repeat(max_label_width)));
            }
            spans.push(Span::styled(line.value, value_style));
            Line::from(spans)
        })
        .collect::<Vec<_>>();
    let paragraph = Paragraph::new(Text::from(lines)).scroll((layout.scroll, 0));
    frame.render_widget(paragraph, rows[0]);
    let footer = Paragraph::new(Text::from(vec![hint_line(bindings(), palette)]));
    frame.render_widget(footer, rows[1]);
}

/// Returns the cursor position for the current create field.
pub(crate) fn cursor_position(area: Rect, create: &State) -> (u16, u16) {
    let popup = crate::app::popup_area(area, 76, 18);
    let inner = crate::app::popup_inner_area(popup);
    let content_height = inner.height.saturating_sub(2);
    let layout = build_layout(create, inner.width, content_height);
    let x = inner.x + layout.cursor_column;
    let y = inner.y + layout.cursor_line.saturating_sub(layout.scroll);
    (x.min(popup.right().saturating_sub(2)), y)
}
