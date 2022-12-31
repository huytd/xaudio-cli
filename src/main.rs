mod youtube;
mod ui;

use std::{io::Result, fmt::Display, collections::HashSet};
use box_drawing::light::HORIZONTAL;
use dotenv::dotenv;
use pancurses::{Window, Input, COLOR_BLUE, init_pair, COLOR_WHITE};
use tokio::sync::mpsc::{Receiver, Sender};
use ui::{App, run};
use xaudio_cli::{ESCAPE_KEY, truncate, TITLE_PADDING, TAB_KEY, BACKSPACE_KEY, ENTER_KEY, get_total_pages, paginate};
use youtube::SearchEntry;

#[derive(Debug)]
enum Command {
    Search(String),
    Play(String)
}

#[derive(Debug)]
enum Message {
    GoToSearch,
    GoToSearchBrowse,
    GoToPlaylist,
    SearchSong,
    AddSelectedToPlaylist,
    RemoveSong,
    NextItem,
    PrevItem,
    NextPage,
    PrevPage,
    InputText(char),
    DeleteText,
    DisplaySearchResult(Vec<SearchEntry>),
    None
}

#[derive(PartialEq, Eq)]
enum AppMode {
    Playing,
    SearchInput,
    SearchBrowse
}

impl Display for AppMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Playing => write!(f, "Now Playing"),
            Self::SearchInput | Self::SearchBrowse => write!(f, "Song Search"),
        }
    }
}

struct MusicApp {
    playing_list: Vec<SearchEntry>,
    search_results: Vec<SearchEntry>,
    current_page: usize,
    page_display_size: usize,
    selected_index: usize,
    keyword: String,
    mode: AppMode,
    subscriber: Sender<Command>,
    loading: bool
}

impl MusicApp {
    pub fn new(tx: Sender<Command>) -> Self {
        Self {
            playing_list: vec![],
            search_results: vec![],
            selected_index: 0,
            current_page: 0,
            page_display_size: 0,
            keyword: String::new(),
            mode: AppMode::Playing,
            subscriber: tx,
            loading: false
        }
    }

    fn switch_mode(&mut self, mode: AppMode, win: &Window) {
        self.mode = mode;
        self.selected_index = 0;
        self.current_page = 0;
        win.clear();
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
        win.printw("[/] Search songs    [x] Remove    [Enter] Play    [Tab] Back to search");
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
        win.printw("[j/k] Up/Down    [<] Previous page    [>] Next page    [/] Search");
    }

    fn draw_list(&self, list: &[SearchEntry], exclude_list: &[SearchEntry], win: &Window) {
        let excluded_ids = exclude_list.iter().map(|entry| entry.id.to_owned()).collect::<HashSet<String>>();
        let (_, screen_width) = win.get_max_yx();
        let total_pages = get_total_pages(list.len(),self.page_display_size);
        let page = paginate(&list, self.current_page, self.page_display_size);

        // clear previous list
        for i in 0..=self.page_display_size as i32 {
            win.mv(2 + i, 0);
            win.clrtoeol();
        }

        // draw the list
        win.mv(2, 0);
        if let Some(page) = page {
            for (i, item) in page.iter().enumerate() {
                let mut attr_flag = pancurses::A_NORMAL;
                if self.selected_index == i {
                    attr_flag |= pancurses::A_REVERSE;
                }
                if excluded_ids.contains(&item.id) {
                    attr_flag |= pancurses::COLOR_PAIR(1);
                }
                win.attron(attr_flag);
                win.printw(format!("{}. {}\n", i + 1 + self.current_page * self.page_display_size, truncate(&item.title, screen_width as usize - TITLE_PADDING)));
                win.attroff(attr_flag);
            }
            win.printw(format!("Page: {}/{}\n", self.current_page + 1, total_pages));
        } else {
            win.printw("Nothing to show. Hit search and add something here.");
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

    fn update(&mut self, win: &Window, msg: Self::Msg) -> bool {
        match msg {
            Message::DisplaySearchResult(result) => {
                self.search_results = result;
                self.switch_mode(AppMode::SearchBrowse, win);
                self.loading = false;
            }
            Message::GoToSearch => {
                self.switch_mode(AppMode::SearchInput, win);
                self.input_clear(win);
            },
            Message::GoToSearchBrowse => {
                self.switch_mode(AppMode::SearchBrowse, win);
            },
            Message::GoToPlaylist => {
                self.switch_mode(AppMode::Playing, win);
            },
            Message::SearchSong => {
                if self.keyword.trim().len() > 0 {
                    _ = self.subscriber.try_send(Command::Search(self.keyword.clone()));
                    self.loading = true;
                }
            },
            Message::AddSelectedToPlaylist => {
                let selected_index = self.selected_index + self.current_page * self.page_display_size;
                let song = &self.search_results[selected_index];
                self.playing_list.push(song.to_owned());
            },
            Message::RemoveSong => {
                self.playing_list.remove(self.selected_index);
            },
            Message::NextPage => {
                let list_len = if self.mode == AppMode::Playing { self.playing_list.len() } else { self.search_results.len() };
                let total_pages = get_total_pages(list_len, self.page_display_size);
                if self.current_page < total_pages - 1 {
                    self.current_page += 1;
                }
                self.selected_index = 0;
            },
            Message::PrevPage => {
                if self.current_page > 0 {
                    self.current_page -= 1;
                }
                self.selected_index = 0;
            },
            Message::NextItem => {
                if self.selected_index < self.page_display_size - 1 {
                    self.selected_index += 1;
                }
            },
            Message::PrevItem => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
            },
            Message::InputText(ch) => {
                self.keyword.push(ch);
            },
            Message::DeleteText => {
                self.input_pop_last(win);
            },
            Message::None => {},
        }
        return true;
    }

    fn input(&mut self, input: Input) -> Self::Msg {
        match self.mode {
            AppMode::Playing => {
                return match input {
                    Input::Character('/') => Message::GoToSearch,
                    Input::Character(TAB_KEY) => Message::GoToSearchBrowse,
                    Input::Character('j') => Message::NextItem,
                    Input::Character('k') => Message::PrevItem,
                    Input::Character('x') => Message::RemoveSong,
                    Input::Character('>') => Message::NextPage,
                    Input::Character('<') => Message::PrevPage,
                    _ => Message::None
                }
            },
            AppMode::SearchInput => {
                return match input {
                    Input::Character(ESCAPE_KEY) => Message::GoToPlaylist,
                    Input::Character(BACKSPACE_KEY) => Message::DeleteText,
                    Input::Character(ENTER_KEY) => Message::SearchSong,
                    Input::Character(ch) => Message::InputText(ch),
                    _ => Message::None
                }
            },
            AppMode::SearchBrowse => {
                return match input {
                    Input::Character(ESCAPE_KEY) | Input::Character('q') => Message::GoToPlaylist,
                    Input::Character('/') => Message::GoToSearch,
                    Input::Character('>') => Message::NextPage,
                    Input::Character('<') => Message::PrevPage,
                    Input::Character('j') => Message::NextItem,
                    Input::Character('k') => Message::PrevItem,
                    Input::Character(ENTER_KEY) => Message::AddSelectedToPlaylist,
                    _ => Message::None
                }
            }
        }
    }

    fn render(&self, win: &Window) {
        self.draw_base_ui(win);

        if self.loading {
            self.draw_loading(win);
        } else {
            match self.mode {
                AppMode::SearchInput => {
                    self.draw_search_box(win);
                }
                AppMode::SearchBrowse => {
                    self.draw_search_instruction(win);
                }
                _ => self.draw_base_instruction(win),
            }
        }

        if let AppMode::Playing = self.mode {
            self.draw_list(&self.playing_list, &[], win);
        } else {
            self.draw_list(&self.search_results, &self.playing_list, win);
        }
    }
}

async fn runtime(mut rx: Receiver<Command>, tx: Sender<Message>) {
    while let Some(msg) = rx.recv().await {
        match msg {
            Command::Search(keyword) => {
                if let Ok(results) = youtube::search_song(&keyword).await {
                    _ = tx.send(Message::DisplaySearchResult(results)).await;
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
