use crate::format::data::Format;
use crate::widget::base::Sender;
use crate::widget::music::{MusicBackend, MusicControl, PlaybackInfo, SongInfo};
use dbus::arg::{Array, RefArg, Variant};
use dbus::blocking::stdintf::org_freedesktop_dbus::Properties;
use dbus::blocking::{BlockingSender, Connection};
use dbus::channel::Sender as DbusSender;
use dbus::Message;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

fn find_player(bus: &Connection) -> Option<String> {
    let m = Message::new_method_call(
        "org.freedesktop.DBus",
        "/",
        "org.freedesktop.DBus",
        "ListNames",
    )
    .unwrap();
    let t = Duration::from_millis(2000);
    let r = bus.send_with_reply_and_block(m, t).unwrap();
    let mut arr: Array<&str, _> = r.get1().unwrap();
    arr.find(|s| s.starts_with("org.mpris.MediaPlayer2."))
        .map(|s| s.to_owned())
}

pub struct MPRISMusic {
    last_value: Arc<RwLock<Format>>,
}

impl MPRISMusic {
    pub fn new() -> MPRISMusic {
        MPRISMusic {
            last_value: Arc::new(RwLock::new(Format::Str(String::new()))),
        }
    }

    fn call_method(&self, method: &str) {
        let bus = Connection::new_session()
            .expect("Could not connect to D-Bus session bus for music control");
        if let Some(player) = find_player(&bus) {
            let m = Message::new_method_call(
                player,
                "/org/mpris/MediaPlayer2",
                "org.mpris.MediaPlayer2.Player",
                method,
            )
            .unwrap();
            let _ = bus.send(m);
        }
    }
}

impl MusicControl for MPRISMusic {
    fn play(&self) {
        self.call_method("Play")
    }

    fn pause(&self) {
        self.call_method("Pause")
    }

    fn play_pause(&self) {
        self.call_method("PlayPause")
    }

    fn stop(&self) {
        self.call_method("Stop")
    }

    fn next(&self) {
        self.call_method("Next")
    }

    fn prev(&self) {
        self.call_method("Previous")
    }
}

impl<F> MusicBackend<F> for MPRISMusic
where
    F: Fn(SongInfo) -> Format + Sync + Send + 'static,
{
    fn current_value(&self) -> Format {
        (*self.last_value).read().unwrap().clone()
    }

    fn spawn_notifier(&mut self, tx: Sender<()>, updater: Arc<Box<F>>) {
        let last_value = self.last_value.clone();
        thread::spawn(move || {
            let bus = Connection::new_session()
                .expect("Could not connect to D-Bus session bus for music info");
            loop {
                if let Some(player) = find_player(&bus) {
                    let properties = &bus.with_proxy(
                        player,
                        "/org/mpris/MediaPlayer2",
                        Duration::from_millis(500),
                    );

                    if let Ok(metadata) =
                        properties.get("org.mpris.MediaPlayer2.Player", "Metadata")
                    {
                        let metadata: HashMap<String, Variant<Box<dyn RefArg>>> = metadata;
                        let get_entry = |t| {
                            metadata
                                .get(t)
                                .and_then(|s| s.as_str())
                                .map(ToString::to_string)
                        };

                        let playback_info = if let Ok(status) =
                            properties.get("org.mpris.MediaPlayer2.Player", "PlaybackStatus")
                        {
                            let s: String = status;
                            let position: i64 = properties
                                .get("org.mpris.MediaPlayer2.Player", "Position")
                                .unwrap_or(-1);
                            let length: i64 = metadata
                                .get("mpris:length")
                                .and_then(RefArg::as_i64)
                                .unwrap_or(-1);
                            Some(PlaybackInfo {
                                playing: s == "Playing",
                                progress: Duration::from_millis((position / 1000) as u64),
                                total: Duration::from_millis((length / 1000) as u64),
                                playlist_index: 0,
                                playlist_total: 0,
                            })
                        } else {
                            None
                        };

                        let state = SongInfo {
                            title: get_entry("xesam:title").unwrap_or_default(),
                            artist: get_entry("xesam:artist").unwrap_or_default(),
                            album: get_entry("xesam:album").unwrap_or_default(),
                            filename: get_entry("xesam:url").unwrap_or_default(),
                            musicbrainz_track: get_entry("xesam:musicBrainzTrackID"),
                            musicbrainz_artist: get_entry("xesam:musicBrainzArtistID"),
                            musicbrainz_album: get_entry("xesam:musicBrainzAlbumID"),
                            playback: playback_info,
                        };

                        let mut writer = last_value.write().unwrap();
                        *writer = (*updater)(state);
                        tx.send(()).unwrap();
                    }
                } else {
                    let mut writer = last_value.write().unwrap();
                    *writer = (*updater)(SongInfo {
                        title: "".to_owned(),
                        artist: "".to_owned(),
                        album: "".to_owned(),
                        filename: "".to_owned(),
                        musicbrainz_track: None,
                        musicbrainz_artist: None,
                        musicbrainz_album: None,
                        playback: None,
                    });
                    tx.send(()).unwrap();
                    thread::sleep(Duration::from_millis(1000)); // more sleepy without player
                }

                // Ideally, this would be smarter than constantly looping...
                // But the only signal in the Player interface is Seeked, no signals for any other
                // state change? Also, would need to detect players disappearing/appearing.
                thread::sleep(Duration::from_millis(500));
            }
        });
    }
}

impl Default for MPRISMusic {
    fn default() -> Self {
        Self::new()
    }
}
