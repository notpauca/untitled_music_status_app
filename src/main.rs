use std::{io::Write, os::unix::net::UnixStream, sync::LazyLock};
use discord_rich_presence::{activity::{ActivityType, Assets, Activity}, DiscordIpc};
use mpd::{Client, Idle, Subsystem};
use reqwest::blocking::multipart::Form;
use tempfile::NamedTempFile;

const CLIENT: LazyLock<reqwest::blocking::Client> = LazyLock::new(|| reqwest::blocking::Client::builder().user_agent("curl/8.7.1").build().unwrap());

fn upload_file(bytes: Vec<u8>) -> anyhow::Result<String> {
    let mut file = NamedTempFile::new()?;
    file.write(bytes.as_slice())?;
    let form = Form::new().file(env!("MULTIPART_NAME"), file.path())?;
    let req = CLIENT.request(reqwest::Method::POST, env!("UPLOAD_LINK")).multipart(form);
    let resp = req.send()?;
    let ret = resp.text()?;
    dbg!(ret.clone());
    Ok(ret)
}

fn main() -> anyhow::Result<()>{
    let mut conn = Client::new(UnixStream::connect(env!("MPD_SOCKET"))?)?;
    let mut presence = discord_rich_presence::DiscordIpcClient::new(env!("DISCORD_CLIENT_ID"));
    presence.connect()?;
    loop {
        if let Some(song) = conn.currentsong()? {
            let song = song;
            let song_name = song.clone().title.unwrap_or(song.clone().file);
            let artist_name = song.clone().artist.unwrap_or("Unknown artist".to_string());
            let activity = Activity::new()
                .activity_type(ActivityType::Listening)
                .details(&song_name)
                .state(&artist_name);
            let image_link = if let Ok(a) = conn.albumart(&song) {
                Some(upload_file(a)?)
            } else {
                None
            };
            if let Some(link) = image_link {
                let activity = activity.assets(Assets::new().large_image(&link));
                presence.set_activity(activity)?;
            } else {
                presence.set_activity(activity)?; //otherwise it cries during compile. but this app's not going to do much while it's running, so it's fine. 
            };
        } else {
            presence.clear_activity()?;
        }
        conn.wait(&[Subsystem::Player])?;
    }
}
