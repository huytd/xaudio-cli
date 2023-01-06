use std::{time::Duration, fs::File, io::{BufReader, BufRead, Write}};

use youtube::SearchEntry;

mod youtube;

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

pub fn playlist_from_file(file_name: &str) -> std::io::Result<Vec<SearchEntry>> {
    let file = File::open(file_name)?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    let mut result = vec![];
    while let Ok(_) = reader.read_line(&mut line) {
        if let Some((id, title)) = line.split_once(" - ") {
            result.push(SearchEntry {
                id: id.to_owned(),
                title: title.to_owned()
            });
        }
    }
    Ok(result)
}

pub fn playlist_to_file(file_name: &str, playlist: &[SearchEntry]) -> std::io::Result<()> {
    let mut file = File::create(file_name)?;
    let output = playlist.iter().map(|song| format!("{} - {}", song.id, song.title)).collect::<Vec<String>>().join("\n");
    write!(file, "{}", output)
}
