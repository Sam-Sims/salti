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
