use std::{io::Result, fmt::Display, collections::HashSet};
use box_drawing::light::HORIZONTAL;
use dotenv::dotenv;
use pancurses::{Window, Input, COLOR_BLUE, init_pair, COLOR_WHITE};
use tokio::sync::mpsc::{Receiver, Sender};
use ui::{App, run};
use youtube::SearchEntry;

mod youtube;
mod ui;

const BACKSPACE_KEY: char = '\u{7f}';
const ESCAPE_KEY: char = '\u{1b}';
const ENTER_KEY: char = '\n';
const TAB_KEY: char = '\t';
const TITLE_PADDING: usize = 12;

/* Utils */

fn truncate(text: &str, len: usize) -> String {
    let char_count = text.chars().count();
    if len > char_count {
        return text.to_owned();
    } else {
        return text.chars().take(len).collect::<String>() + "...";
    }
}

/* --------------- */

#[derive(Debug)]
enum Command {
    Search(String),
    Play(String)
}

#[derive(Debug)]
enum Message {
    SearchResult(Vec<SearchEntry>),
}

enum MusicAppMode {
    Playing,
    SearchInput,
    SearchBrowse
}

impl Display for MusicAppMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Playing => write!(f, "Now Playing"),
            Self::SearchInput | Self::SearchBrowse => write!(f, "Song Search"),
        }
    }
}

struct MusicApp {
    playing_list: Vec<SearchEntry>,
    playing_page: usize,
    search_results: Vec<SearchEntry>,
    search_page: usize,
    selected_index: usize,
    page_display_size: usize,
    keyword: String,
    mode: MusicAppMode,
    subscriber: Sender<Command>,
    loading: bool
}

impl MusicApp {
    pub fn new(tx: Sender<Command>) -> Self {
        Self {
            playing_list: vec![],
            playing_page: 0,
            search_page: 0,
            selected_index: 0,
            page_display_size: 0,
            search_results: vec![],
            keyword: String::new(),
            mode: MusicAppMode::Playing,
            subscriber: tx,
            loading: false
        }
    }

    fn input_pop_last(&mut self, win: &Window) {
        let (cy, cx) = win.get_cur_yx();
        win.mvprintw(cy, cx - 1, "   ");
        self.keyword.pop();
    }

    fn input_clear(&mut self, win: &Window) {
        let (cy, cx) = win.get_cur_yx();
        let len = self.keyword.len() as i32;
        win.mv(cy, cx - len);
        win.clrtoeol();
        self.keyword.clear();
    }

    fn input_mode_playing(&mut self, input: Input, win: &Window) {
        match input {
            Input::Character('/') => {
                self.mode = MusicAppMode::SearchInput;
                self.keyword = String::new();
                win.clear();
            }
            Input::Character(TAB_KEY) => {
                self.mode = MusicAppMode::SearchBrowse;
                win.clear();
            }
            Input::Character('j') => {
                if self.selected_index < self.page_display_size - 1 {
                    self.selected_index += 1;
                }
            }
            Input::Character('k') => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
            }
            Input::Character('>') => {
                let total_playlist_page = self.playing_list.len() / self.page_display_size;
                if self.playing_page < total_playlist_page - 1 {
                    self.playing_page += 1;
                }
            }
            Input::Character('<') => {
                if self.playing_page > 0 {
                    self.playing_page -= 1;
                }
            }
            _ => {}
        }
    }

    fn input_mode_search_input(&mut self, input: Input, win: &Window) {
        match input {
            Input::Character(ESCAPE_KEY) => {
                self.mode = MusicAppMode::Playing;
                win.clear();
            }
            Input::Character(BACKSPACE_KEY) => {
                self.input_pop_last(win);
            }
            Input::Character(ENTER_KEY) => {
                if self.keyword.trim().len() > 0 {
                    _ = self.subscriber.try_send(Command::Search(self.keyword.clone()));
                    self.loading = true;
                }
            }
            Input::Character(ch) => {
                self.keyword.push(ch);
            }
            _ => {}
        }
    }

    fn input_mode_search_browse(&mut self, input: Input, win: &Window) {
        match input {
            Input::Character(ESCAPE_KEY) | Input::Character('q') => {
                self.mode = MusicAppMode::Playing;
                win.clear();
            }
            Input::Character('i') => {
                self.input_clear(win);
                self.mode = MusicAppMode::SearchInput;
            }
            Input::Character('>') => {
                let total_search_page = self.search_results.len() / self.page_display_size;
                if self.search_page < total_search_page - 1 {
                    self.search_page += 1;
                }
            }
            Input::Character('<') => {
                if self.search_page > 0 {
                    self.search_page -= 1;
                }
            }
            Input::Character('j') => {
                if self.selected_index < self.page_display_size - 1 {
                    self.selected_index += 1;
                }
            }
            Input::Character('k') => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
            }
            Input::Character(ENTER_KEY) => {
                let selected_index = self.selected_index + self.search_page * self.page_display_size;
                let song = &self.search_results[selected_index];
                self.playing_list.push(song.to_owned());
            }
            _ => {}
        }
    }

    fn draw_base_ui(&self, win: &Window) {
        let (screen_height, screen_width) = win.get_max_yx();
        let horizontal_line = std::iter::repeat(HORIZONTAL).take(screen_width as usize).collect::<String>();
        win.mvprintw(0, 0, format!("{}", self.mode));
        win.mvprintw(1, 0, &horizontal_line);
        win.mvprintw(screen_height - 2, 0, &horizontal_line);
    }

    fn draw_base_instruction(&self, win: &Window) {
        let (screen_height, _) = win.get_max_yx();
        win.mv(screen_height - 1, 1);
        win.clrtoeol();
        win.printw("[/] Search songs");
    }

    fn draw_loading(&self, win: &Window) {
        let (screen_height, _) = win.get_max_yx();
        win.mv(screen_height - 1, 1);
        win.clrtoeol();
        win.printw("Loading...");
    }

    fn draw_search_box(&self, win: &Window) {
        let (screen_height, _) = win.get_max_yx();
        win.mv(screen_height - 1, 1);
        win.clrtoeol();
        win.mvprintw(screen_height - 1, 1, format!("Search: {}█", self.keyword));
    }

    fn draw_search_instruction(&self, win: &Window) {
        let (screen_height, _) = win.get_max_yx();
        win.mv(screen_height - 1, 1);
        win.clrtoeol();
        win.printw("[j/k] Up/Down    [<] Previous page    [>] Next page    [i] Search");
    }

    fn draw_list(&self, list: &[SearchEntry], exclude_list: &[SearchEntry], current_page: usize, selected_index: usize, win: &Window) {
        let excluded_ids = exclude_list.iter().map(|entry| entry.id.to_owned()).collect::<HashSet<String>>();
        let (_, screen_width) = win.get_max_yx();
        if list.len() > 0 {
            let total_pages = list.len() / self.page_display_size;
            let page_start = current_page * self.page_display_size;
            let page_end = page_start + self.page_display_size;
            let items = if let Some(_) = list.get(page_end) { &list[page_start..page_end] } else { list };
            win.mv(2, 0);
            for (i, item) in items.iter().enumerate() {
                let mut attr_flag = pancurses::A_NORMAL;
                if selected_index == i {
                    attr_flag |= pancurses::A_REVERSE;
                }
                if excluded_ids.contains(&item.id) {
                    attr_flag |= pancurses::COLOR_PAIR(1);
                }
                win.attron(attr_flag);
                win.printw(format!("{}. {}\n", i + 1 + page_start, truncate(&item.title, screen_width as usize - TITLE_PADDING)));
                win.clrtoeol();
                win.attroff(attr_flag);
            }
            win.printw(format!("Page: {}/{}\n", current_page + 1, total_pages));
            win.clrtoeol();
        } else {
            win.mv(2, 0);
            win.printw("Nothing here ;(");
        }
    }
}

impl App for MusicApp {
    type Msg = Message;

    fn init(&mut self, win: &Window) {
        let (screen_height, _) = win.get_max_yx();
        self.page_display_size = (screen_height - 6) as usize;

        init_pair(0, COLOR_WHITE, 0);
        init_pair(1, COLOR_BLUE, 0);
    }

    fn update(&mut self, _: &Window) {}

    fn input(&mut self, input: Input, win: &Window) -> bool {
        match self.mode {
            MusicAppMode::Playing => {
                self.input_mode_playing(input, win);
            },
            MusicAppMode::SearchInput => {
                self.input_mode_search_input(input, win);
            },
            MusicAppMode::SearchBrowse => {
                self.input_mode_search_browse(input, win);
            }
        }
        return true;
    }

    fn render(&self, win: &Window) {
        self.draw_base_ui(win);

        if self.loading {
            self.draw_loading(win);
        } else {
            match self.mode {
                MusicAppMode::SearchInput => {
                    self.draw_search_box(win);
                }
                MusicAppMode::SearchBrowse => {
                    self.draw_search_instruction(win);
                }
                _ => self.draw_base_instruction(win),
            }
        }

        if let MusicAppMode::Playing = self.mode {
            self.draw_list(&self.playing_list, &[], self.playing_page, self.selected_index, win);
        } else {
            self.draw_list(&self.search_results, &self.playing_list, self.search_page, self.selected_index, win);
        }
    }

    fn subscription(&mut self, msg: Self::Msg) {
        match msg {
            Message::SearchResult(result) => {
                self.search_results = result;
                self.mode = MusicAppMode::SearchBrowse;
            }
        }
        self.loading = false;
    }
}

async fn runtime(mut rx: Receiver<Command>, tx: Sender<Message>) {
    while let Some(msg) = rx.recv().await {
        match msg {
            Command::Search(keyword) => {
                if !keyword.is_empty() {
                    if let Ok(results) = youtube::search_song(&keyword).await {
                        _ = tx.send(Message::SearchResult(results)).await;
                    }
                }
            },
            _ => {}
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel::<Command>(1);
    let (msg_tx, msg_rx) = tokio::sync::mpsc::channel::<Message>(1);
    let app = MusicApp::new(cmd_tx);
    tokio::spawn(runtime(cmd_rx, msg_tx));
    run(app, false, msg_rx);
    Ok(())
}
