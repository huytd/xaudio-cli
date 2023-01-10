mod utils;
mod youtube;
mod ui;
mod mpv;

use std::{io::Result, fmt::Display, collections::HashSet, thread, time::{Duration, Instant}};
use box_drawing::light::HORIZONTAL;
use dotenv::dotenv;
use mpv::MpvClient;
use pancurses::{Window, Input, COLOR_BLUE, init_pair, COLOR_WHITE};
use tokio::{sync::mpsc::{Receiver, Sender}, select};
use ui::{App, run};
use utils::{ESCAPE_KEY, truncate, TITLE_PADDING, TAB_KEY, BACKSPACE_KEY, ENTER_KEY, get_total_pages, paginate, display_time, create_index_queue};
use youtube::SongEntry;

use crate::utils::{save_playlist, read_playlist};

// TODO:
// 1. BUG - Add duplicate item into playlist
// 2. FEA - Support multiple playlists
// 3. FEA - Shuffle Songs

#[derive(Debug)]
enum Command {
    Search(String),
    Play(String),
    SavePlaylist(Vec<SongEntry>)
}

#[derive(Debug)]
enum Message {
    // Main UI
    GoToSearch,
    GoToSearchBrowse,
    GoToPlaylist,
    // Searching and Listing
    SearchSong,
    AddSelectedToPlaylist,
    RemoveSong,
    NextItem,
    PrevItem,
    NextPage,
    PrevPage,
    // Playback
    PlaySelected,
    NextSong,
    PrevSong,
    ToggleShuffle,
    // Input box
    InputText(char),
    DeleteText,
    // Runtime messages
    DisplaySearchResult(Vec<SongEntry>),
    SongStarted(Instant),
    SongStopped(String),
    SongDuration(Duration),
    // Other
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
    mode: AppMode,
    current_playlist: Vec<SongEntry>,
    search_results: Vec<SongEntry>,
    current_page: usize,
    page_display_size: usize,
    selected_index: usize,
    keyword: String,
    loading: bool,
    subscriber: Sender<Command>,
    song_duration: Duration,
    playing: bool,
    playing_index: usize,
    last_started: Instant,
    play_queue: Vec<usize>,
    queue_index: usize,
    is_shuffle: bool
}

impl MusicApp {
    pub fn new(playlist: Vec<SongEntry>, tx: Sender<Command>) -> Self {
        let playlist_len = playlist.len();
        Self {
            mode: AppMode::Playing,
            current_playlist: playlist,
            search_results: vec![],
            current_page: 0,
            page_display_size: 0,
            selected_index: 0,
            keyword: String::new(),
            loading: false,
            subscriber: tx,
            playing: false,
            playing_index: 0,
            last_started: Instant::now(),
            song_duration: Duration::default(),
            play_queue: create_index_queue(playlist_len, false),
            queue_index: 0,
            is_shuffle: false
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

    fn play_selected_song(&mut self) {
        let selected_index = self.selected_index + self.current_page * self.page_display_size;
        let song = &self.current_playlist[selected_index];
        _ = self.subscriber.try_send(Command::Play(song.id.to_owned()));
        self.playing_index = self.selected_index;
    }

    fn play_next_song(&mut self) {
        if self.queue_index < self.play_queue.len() - 1 {
            self.queue_index += 1;
        } else {
            // rebuild the play queue if needed
            self.play_queue = create_index_queue(self.current_playlist.len(), self.is_shuffle);
            self.queue_index = 0;
        }
        self.selected_index = self.play_queue[self.queue_index];
        self.play_selected_song();
    }

    fn play_prev_song(&mut self) {
        if self.queue_index > 0 {
            self.queue_index -= 1;
        }
        self.selected_index = self.play_queue[self.queue_index];
        self.play_selected_song();
    }

    fn draw_base_ui(&self, win: &Window) {
        let (screen_height, screen_width) = win.get_max_yx();
        let horizontal_line = std::iter::repeat(HORIZONTAL).take(screen_width as usize).collect::<String>();
        win.mv(0, 0);
        win.clrtoeol();
        if self.playing {
            let played_duration = display_time(Instant::now().duration_since(self.last_started));
            let total_duration = display_time(self.song_duration);
            let current_song = &self.current_playlist[self.playing_index];
            let shuffle_icon = if self.is_shuffle { "~" } else { "" };
            win.mvprintw(0, 0, format!("▶{} {} - {} / {}", shuffle_icon, truncate(&current_song.title, 60), played_duration, total_duration));
        } else {
            win.mvprintw(0, 0, format!("{}", self.mode));
        }
        win.mvprintw(1, 0, &horizontal_line);
        win.mvprintw(screen_height - 2, 0, &horizontal_line);
    }

    fn draw_base_instruction(&self, win: &Window) {
        let (screen_height, _) = win.get_max_yx();
        win.mv(screen_height - 1, 1);
        win.clrtoeol();
        win.printw(format!("[/] Search  [x] Remove  [Enter] Play  [n/p] Next/Prev  [s] Shuffle {}  [Tab] Back to search",
            if self.is_shuffle { "ON" } else { "OFF" }
        ));
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

    fn draw_list(&self, list: &[SongEntry], exclude_list: &[SongEntry], win: &Window) {
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
            },
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
                self.current_playlist.push(song.to_owned());
                _ = self.subscriber.try_send(Command::SavePlaylist(self.current_playlist.to_owned()));
                self.play_queue = create_index_queue(self.current_playlist.len(), self.is_shuffle);
            },
            Message::RemoveSong => {
                self.current_playlist.remove(self.selected_index);
                _ = self.subscriber.try_send(Command::SavePlaylist(self.current_playlist.to_owned()));
                self.play_queue = create_index_queue(self.current_playlist.len(), self.is_shuffle);
            },
            Message::NextPage => {
                let list_len = if self.mode == AppMode::Playing { self.current_playlist.len() } else { self.search_results.len() };
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
            Message::PlaySelected => {
                self.play_selected_song();
            },
            Message::SongStarted(current_time) => {
                self.playing = true;
                self.last_started = current_time;
            },
            Message::SongStopped(reason) => {
                self.playing = false;
                if reason.eq("eof") {
                    self.play_next_song();
                }
            },
            Message::SongDuration(duration) => {
                self.song_duration = duration;
            },
            Message::NextSong => {
                self.play_next_song();
            },
            Message::PrevSong => {
                self.play_prev_song();
            },
            Message::ToggleShuffle => {
                self.is_shuffle = !self.is_shuffle;
                self.play_queue = create_index_queue(self.current_playlist.len(), self.is_shuffle);
            },
            Message::None => {},
        }
        return true;
    }

    fn input(&mut self, input: Input) -> Self::Msg {
        match self.mode {
            AppMode::Playing => {
                return match input {
                    Input::Character(ENTER_KEY) => Message::PlaySelected,
                    Input::Character('/') => Message::GoToSearch,
                    Input::Character(TAB_KEY) => Message::GoToSearchBrowse,
                    Input::Character('j') => Message::NextItem,
                    Input::Character('k') => Message::PrevItem,
                    Input::Character('x') => Message::RemoveSong,
                    Input::Character('>') => Message::NextPage,
                    Input::Character('<') => Message::PrevPage,
                    Input::Character('n') => Message::NextSong,
                    Input::Character('p') => Message::PrevSong,
                    Input::Character('s') => Message::ToggleShuffle,
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
            let highlight_playing = if self.playing {
                vec![self.current_playlist[self.playing_index].clone()]
            } else {
                vec![]
            };
            self.draw_list(&self.current_playlist, &highlight_playing, win);
        } else {
            self.draw_list(&self.search_results, &self.current_playlist, win);
        }
    }
}

async fn runtime(mut rx: Receiver<Command>, tx: Sender<Message>) {
    let mut mpv = MpvClient::new().await;
    loop {
        select! {
            app_command = rx.recv() => {
                if let Some(msg) = app_command {
                    match msg {
                        Command::Search(keyword) => {
                            if let Ok(results) = youtube::search_song(&keyword).await {
                                _ = tx.send(Message::DisplaySearchResult(results)).await;
                            }
                        }
                        Command::Play(song_id) => {
                            let song_duration = youtube::get_song_duration(&song_id).await.unwrap_or_default();
                            _ = tx.send(Message::SongDuration(song_duration)).await;
                            mpv.load_song(format!("https://www.youtube.com/watch?v={}", song_id).as_str()).await;
                            mpv.play().await;
                        }
                        Command::SavePlaylist(current_playlist) => {
                            _ = save_playlist(&current_playlist);
                        }
                    }
                }
            },
            mpv_event = mpv.recv() => {
                if let Ok(event) = mpv_event {
                    match event {
                        mpv::MpvEvent::StartFile => {
                            _ = tx.send(Message::SongStarted(Instant::now())).await;
                        },
                        mpv::MpvEvent::EndFile(reason) => {
                            _ = tx.send(Message::SongStopped(reason)).await;
                        },
                        mpv::MpvEvent::Unknown(_event) => {}
                    }
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    tokio::spawn(async move {
        MpvClient::start_server().await;
    });
    // Want to know why a 500ms delay? It's a long story.
    // Once upon a time, there was a process called "mpv" spawned
    // after the dotenv().ok() statement. It carries the responsibility
    // of being an RPC server that our runtime will be connected to.
    // This process takes a few milliseconds to start, if we just start
    // the MusicApp right away, the connection would be failed.
    // Hence, we wait 500ms.
    thread::sleep(Duration::from_millis(500));
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel::<Command>(1);
    let (msg_tx, msg_rx) = tokio::sync::mpsc::channel::<Message>(1);

    let playlist = read_playlist().unwrap_or(vec![]);
    let app = MusicApp::new(playlist, cmd_tx);
    tokio::spawn(runtime(cmd_rx, msg_tx));
    run(app, false, msg_rx);
    Ok(())
}
