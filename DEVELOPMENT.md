# Technical Details

This note outlined some details about the implementation of the Xaudio CLI app.

## The UI

To render on the terminal, we're using the [pancurses](https://crates.io/crates/pancurses) 
crate, it is a library that provide a cross platform API that wrapped around the _ncurses_ library.

### Elm-like Architecture

The [src/ui.rs](src/ui.rs) module provide an `App` trait, which implements an Elm-like architecture, 
to help handling the lifecycle of a terminal UI application. 

```rust
pub trait App {
    type Msg;
    fn init(&mut self, win: &Window);
    fn update(&mut self, win: &Window, msg: Self::Msg) -> bool;
    fn input(&mut self, input: Input) -> Self::Msg;
    fn render(&self, win: &Window);
}
```

<img width="743" alt="image" src="https://user-images.githubusercontent.com/613943/210499293-04f35339-a474-4798-b1ac-4151ef0c5a9b.png">

An application that implemented `App` trait will work in the following steps:

- Initialized the application with the `App::init()` method
- UI rendering logic will be implemented inside the `App::render()` method. There should be no data 
manipulation happen in the rendering step, hence, the receiver of this method is an immutable `&self`.
- Input events like keyboard will be handled in the `App::input()` method. This method returns a `Msg` 
value. A `Msg` can be used as a state manipulation signal.
- All the `Msg` signals will be handled by the `App::update()` method, which will transform the app's 
state. New app states will be rendered out in the `App::render()` method.

### The MusicApp struct

The Xaudio CLI application is defined by the `MusicApp` struct, which implements the `App` trait.

The `Message` enum defined all the UI signals that the `MusicApp` will be interacted with, for example:

- The `GoToSearch`, `GoToSearchBrowse`, `GoToPlaylist` messages will toggle the different `AppMode`. 
Each mode is each screen in the app. We have 3 main screens: _Playlist_, _Search input_ and _Search browse_.
- The `InputText(char)`, `DeleteText` messages are used for handling text input in the _Search input_ screen.
- The `PlaySelected`, `NextSong`, `PrevSong` messages are the playback signal that will be sent to the runtime 
method to interact with MPV.
- The `DisplaySearchResult(Vec<SongEntry>)`, `SongStarted(Instant)`, `SongStopped(String)` are the messages 
that will be sent back to the `MusicApp` from the runtime method. 

The UI rendering logic are being implemented in the `MusicApp::render()` method, but different part of the UI 
are splitted into each smaller render method like `draw_loading()`, `draw_search_box()`, `draw_list()`,...

### Communicating with external tasks

As you can see, everything implemented in the `MusicApp` are for the UI only. Tasks like making API call to 
Youtube or communicating with MPV to play audio are considered external tasks. To handle these tasks, we have
the `runtime()` method running in a separated thread.

The `MusicApp` communicates with the `runtime()` thread via two channels `(Receiver<Command>, Sender<Message>)`.

<img width="926" alt="image" src="https://user-images.githubusercontent.com/613943/210498959-9929ada4-a173-4539-8c90-872e1c3b6198.png">

When ever we need to trigger some external tasks, the `MusicApp` will dispatch a `Command` via the channel:

```rust
_ = self.subscriber.try_send(Command::Play(song.id.to_owned()));
```

And when the `runtime` need to send some information back to the `MusicApp`, we send the `Message` back:

```rust
_ = tx.send(Message::SongStarted(Instant::now())).await;
```

This message will then be handled by the `MusicApp::update()` method.

## MPV client

The main functionality of `MusicApp` is to interact with the Youtube API to search sonsg and keeping a playlist.
To actually play music from a Youtube URL, we're using MPV.

To communicate with the MPV process, we first spawn the `mpv` application as a [JSON IPC](https://mpv.io/manual/stable/#json-ipc) server with the 
`MpvClient::start_server()` method. Then open a new `UnixStream` connection to MPV's socket server:

```rust
let stream = UnixStream::connect("/tmp/mpv-socket").await.expect("Cannot connect to MPV");
```

Although MPV supports playlist, to make it simpler, we only load one song at a time to play. The playback process
for a song would be described as:

1. Get the selected song information and its Youtube ID
2. Construct a Youtube URL from that ID
3. Send a `loadfile` command to MPV to load that URL
4. Play the loaded song in MPV

See `MusicApp::play_selected_song()` method for the implementation.

<img width="1083" alt="image" src="https://user-images.githubusercontent.com/613943/210510024-ce73932a-dd12-4a52-b33d-5bc2a9eb5e44.png">

From the `MusicApp`, a `Command::Play(song-id)` command will be sent to the `runtime` thread to communicate with MPV. When MPV 
start to play the music, a message called `Message::SongStarted(current-time)` will be sent back to `MusicApp`.

When a song is finished, an message called `Message::SongStopped(reason)` will be sent to `MusicApp`, with `reason` being the 
`"eof"` string. By receiving this, we will know that it's time to play the next song in the playlist, the `MusicApp::play_next_song()` method
will be called to handle this.

<img width="1062" alt="image" src="https://user-images.githubusercontent.com/613943/210510630-ed9be5a1-9f75-486f-8c56-7e53d98764b7.png">

Currently, only a small set of MPV commands/events are being implemented. The list may or may not be extended in the future, depends on what 
is needed. For a full list of commands/events, please check the following links:

- https://mpv.io/manual/stable/#list-of-input-commands
- https://mpv.io/manual/stable/#list-of-events

The implementation's of [src/mpv.rs](src/mpv.rs) module are heavily inspired by the @joleeee's [mpvi](https://github.com/joleeee/mpvi) project.

## Song shuffling

Song shuffling is a very interesting problem.

The simplest way to implement song shuffling is to generate a random index everytime the user switch to a next or previous song.
There are a couple of problems with this approach:

- The user cannot go back to the previous song because the index is randomized on every action
- Related to the above point, there's no way to track the listening history
- It is very likely that some songs will be played more than once, while some songs will never get played

For a better approach, we will keep the list of songs to play in a list called `play_queue`. This queue is built by shuffling around the
index of the songs, with the help of the `rand` crate:

```rust
let mut rng = rand::thread_rng();
let mut play_queue: Vec<usize> = (0..len).collect();
if shuffle {
    play_queue.shuffle(&mut rng);
}
```

By doing this, we will always have a list of song ids shuffled in either random or linear order depending on the play mode (`is_shuffle: bool`).

And with this approach, we will be able to implement the three points mentioned above: we can keep track of the played song, so we can implement the next/prev feature correctly, and all songs in the playlist are guaranteed to be played at least once per shuffle session.