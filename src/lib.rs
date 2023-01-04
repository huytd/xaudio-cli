use std::time::Duration;

pub const BACKSPACE_KEY: char = '\u{7f}';
pub const ESCAPE_KEY: char = '\u{1b}';
pub const ENTER_KEY: char = '\n';
pub const TAB_KEY: char = '\t';
pub const TITLE_PADDING: usize = 12;

pub fn truncate(text: &str, len: usize) -> String {
    let char_count = text.chars().count();
    if len > char_count {
        return text.to_owned();
    } else {
        return text.chars().take(len).collect::<String>() + "…";
    }
}

pub fn get_total_pages(len: usize, page_size: usize) -> usize {
    len / page_size + if len % page_size == 0 { 0 } else { 1 }
}

pub fn paginate<'a, T>(list: &'a [T], page: usize, page_size: usize) -> Option<&'a [T]> {
    let start = page * page_size;
    if start < list.len() {
        let end = start + page_size;
        return Some(if &list[start..].len() < &page_size {
            &list[start..]
        } else {
            &list[start..end]
        });
    }
    None
}

pub fn display_time(dur: Duration) -> String {
    let sec = dur.as_secs() % 60;
    let min = (dur.as_secs() / 60) % 60;
    let hrs = (dur.as_secs() / 60) / 60;
    format!("{:02}:{:02}:{:02}", hrs, min, sec)
}
