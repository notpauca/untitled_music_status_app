use std::{io::Write, sync::LazyLock, time::{SystemTime, UNIX_EPOCH}};
use discord_rich_presence::{activity::{ActivityType, Assets, Activity, StatusDisplayType, Timestamps}, DiscordIpc, DiscordIpcClient};
use mpd::{Client, Idle, State, Subsystem};
use reqwest::blocking::multipart::Form;
use tempfile::NamedTempFile;

const CLIENT: LazyLock<reqwest::blocking::Client> = LazyLock::new(|| reqwest::blocking::Client::builder().user_agent("curl/8.7.1").build().unwrap());

fn upload_file(bytes: Vec<u8>) -> anyhow::Result<String> {
    let mut file = NamedTempFile::new()?;
    file.write(bytes.as_slice())?;
    let form = Form::new().file(env!("MULTIPART_NAME"), file.path())?;
    let req = CLIENT.request(reqwest::Method::POST, env!("UPLOAD_LINK")).multipart(form);
    let resp = req.send()?;
    Ok(resp.text()?)
}

fn presence_set(presence: &mut DiscordIpcClient, activity: Option<Activity>) {
    match activity.clone() {
        Some(activity) => presence.set_activity(activity),
        None => presence.clear_activity()
    }.unwrap_or_else(|_| {
        let _ = presence.reconnect();
        presence_set(presence, activity)
    });
}

fn main() -> anyhow::Result<()>{
    let mut conn = Client::connect(env!("MPD_ADDRESS"))?;
    let mut presence = DiscordIpcClient::new(env!("DISCORD_CLIENT_ID"));
    presence.connect()?;
    loop {
        let state = conn.status()?.state;
        match conn.currentsong()? {
            Some(song) => {
                let song_name = format!("{} {}", song.title.clone().unwrap_or(song.file.clone()), if state==State::Pause {"(Paused)"} else {""} );
                let artist_name = song.artist.clone().unwrap_or("Unknown artist".to_string());
                let album_art = conn.albumart(&song).ok().map(|a| upload_file(a).unwrap());
                let queued_operation = Activity::new().activity_type(ActivityType::Listening)
                        .details(&song_name)
                        .state(&artist_name)
                        .status_display_type(StatusDisplayType::Details)
                        .timestamps(
                            if state == State::Play {
                                let Some((in_song, song_length)) = conn.status()?.time else { unreachable!() };
                                Timestamps::new()
                                    .end((SystemTime::now().duration_since(UNIX_EPOCH)?+song_length-in_song).as_secs() as i64)
                                    .start((SystemTime::now().duration_since(UNIX_EPOCH)?-in_song).as_secs() as i64)
                            } else {
                                Timestamps::new()
                            }
                        )
                        .assets(
                            if let Some(album_art) = &album_art {
                                Assets::new().large_image(&album_art)
                            } else {
                                Assets::new()
                            }
                        );
                presence_set(&mut presence, Some(queued_operation));
            },
            None => presence_set(&mut presence, None)
        }
        conn.wait(&[Subsystem::Player])?;
    }
}

