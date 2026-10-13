pub const PLAYING_ALBUM_BAR: &str = "▎ ";

/// `select_next`/`select_previous` can leave the selection past the end
/// (e.g. `usize::MAX` from Up with nothing selected), and highlighting is
/// computed before the widget renders, so it has to be clamped up front.
pub fn clamp_selection<S>(
    state: &mut S,
    selected: fn(&S) -> Option<usize>,
    select: fn(&mut S, Option<usize>),
    len: usize,
) {
    match selected(state) {
        Some(_) if len == 0 => select(state, None),
        Some(index) if index >= len => select(state, Some(len - 1)),
        _ => {}
    }
}

pub fn format_duration(seconds: i64) -> String {
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}
