use serde_json::{json, Value};
use tokio::{net::{UnixStream, unix::{OwnedWriteHalf, OwnedReadHalf}}, io::{BufReader, AsyncBufReadExt, AsyncWriteExt}};

#[derive(Debug)]
pub enum MpvEvent {
    StartFile,
    EndFile,
    Unknown
}

pub struct MpvClient {
    reader: BufReader<OwnedReadHalf>,
    writer: OwnedWriteHalf,
}

impl MpvClient {
    pub async fn start_server() {
        _ = tokio::process::Command::new("mpv")
            .arg("--input-ipc-server=/tmp/mpv-socket")
            .arg("--no-terminal")
            .arg("--no-video")
            .arg("--idle")
            .spawn()
            .expect("Cannot start MPV server")
            .wait()
            .await;
    }

    pub async fn new() -> Self {
        let stream = UnixStream::connect("/tmp/mpv-socket").await.expect("Cannot connect to MPV");
        let (read, write) = stream.into_split();
        let reader = BufReader::new(read);
        Self {
            reader,
            writer: write
        }
    }

    pub async fn send(&mut self, args: Vec<&str>) {
        _ = self.writer
            .write_all(json!({
                "command": args
            }).to_string().as_bytes())
            .await;
        _ = self.writer
            .write_u8('\n' as u8)
            .await;
    }

    pub async fn recv(&mut self) -> std::io::Result<MpvEvent> {
        let mut buf = String::new();
        self.reader.read_line(&mut buf).await?;
        let parsed = serde_json::from_str::<Value>(&buf)?;
        Ok(match parsed["event"].as_str() {
            Some("start-file") => MpvEvent::StartFile,
            Some("end-file") => MpvEvent::EndFile,
            _ => MpvEvent::Unknown
        })
    }

    pub async fn load_song(&mut self, url: &str) {
        self.send(vec!["loadfile", url, "append"]).await;
    }

    pub async fn play(&mut self) {
        self.send(vec!["playlist-play-index", "0"]).await;
    }

    pub async fn pause(&mut self) {
        self.send(vec!["set", "pause", "yes"]).await;
    }

    pub async fn unpause(&mut self) {
        self.send(vec!["set", "pause", "no"]).await;
    }
}
