use std::{time::Duration, fs::File, io::{BufReader, BufRead, Write}};
use rand::Rng;
use crate::youtube::SongEntry;

pub const BACKSPACE_KEY: char = '\u{7f}';
pub const ESCAPE_KEY: char = '\u{1b}';
pub const ENTER_KEY: char = '\n';
pub const TAB_KEY: char = '\t';
pub const TITLE_PADDING: usize = 12;
pub const HOME_DIR: &str = env!("HOME");
pub const PLAYLIST_FILE_PATH: &str = "/.xaudio-playlist";

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

pub fn read_playlist() -> std::io::Result<Vec<SongEntry>> {
    let file_name = format!("{}{}", HOME_DIR, PLAYLIST_FILE_PATH);
    let file = File::open(file_name)?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    let mut result = vec![];
    while let Ok(bytes) = reader.read_line(&mut line) {
        if bytes == 0 {
            break;
        }
        if let Some((id, title)) = line.split_once(" - ") {
            result.push(SongEntry {
                id: id.to_owned(),
                title: title.trim().to_owned()
            });
        }
        line.clear();
    }
    Ok(result)
}

pub fn save_playlist(playlist: &[SongEntry]) -> std::io::Result<()> {
    let file_name = format!("{}{}", HOME_DIR, PLAYLIST_FILE_PATH);
    let mut file = File::create(file_name)?;
    playlist.iter().map(|song| format!("{} - {}", song.id, song.title)).for_each(|line| {
        _ = writeln!(file, "{}", line);
    });
    Ok(())
}

pub fn create_index_queue(len: usize, shuffle: bool) -> Vec<usize> {
    let mut rng = rand::thread_rng();
    let mut ret = vec![];
    while ret.len() < len {
        let val = if shuffle { rng.gen_range(0..len) } else { *ret.last().unwrap_or(&0) };
        if !ret.contains(&val) {
            ret.push(val);
        }
    }
    return ret;
}