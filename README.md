# Xaudio CLI

Xaudio CLI is a remake version of [xaudio](https://github.com/huytd/xaudio), focusing on
CLI interface.

![](https://user-images.githubusercontent.com/613943/210482625-d01b016e-33c4-43b0-84ac-536137e33bdc.png)

## System requirements

Xaudio CLI uses [MPV](https://mpv.io) as a backend, so, make sure you have MPV installed:

```
brew instal mpv
```

To compile from source, you'll also need the Rust compiler, which is obvious.

## How to run

To run, you'll need to create a `.env` file and put in your Youtube API key:

```
YOUTUBE_API_KEY=<your-key-here>
```

Then run the application with:

```
make
```

## How to use

The app will start in _Playlist_ mode, in this mode, you can:
- Hit `/` to search for songs
- Use `j` and `k` to navigate up and down
- Use `<` and `>` to switch between pages
- Hit `Enter` to play a song
- Use `n` and `p` to play next/previous song
- Hit `Tab` to go back to the previous search result

In the _Search_ mode, you can type the song name to search and navigate with the 
same keybinding as the _Playlist_ mode. You can also hit `ESC` to go back to the
_Playlist_ mode.

## Technical Details

Please refer to [DEVELOPMENT.md](DEVELOPMENT.md) for more about the technical details:

- The Elm-like Architecture ([src/ui.rs](src/ui.rs), [MusicApp](https://github.com/huytd/xaudio-cli/blob/main/src/main.rs#L232) struct)
- mpv JSON IPC client ([src/mpv.rs](src/mpv.rs))
- Youtube API v3 ([src/youtube.rs](src/youtube.rs))
