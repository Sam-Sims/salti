#[must_use]
pub fn split_pinned_rows(max_rows: usize, pinned_visible_rows: usize) -> (usize, bool, usize) {
    let has_pins = pinned_visible_rows > 0 && max_rows > 0;
    let pinned_rows = if has_pins {
        pinned_visible_rows.min(max_rows.saturating_sub(1))
    } else {
        0
    };
    let unpinned_rows = max_rows.saturating_sub(pinned_rows + usize::from(has_pins));

    (pinned_rows, has_pins, unpinned_rows)
}

#[must_use]
pub fn truncate_label(value: &str, width: usize) -> String {
    let char_count = value.chars().count();
    if char_count <= width {
        return value.to_string();
    }

    if width <= 3 {
        return value.chars().take(width).collect();
    }

    let mut text: String = value.chars().take(width - 3).collect();
    text.push_str("...");
    text
}
