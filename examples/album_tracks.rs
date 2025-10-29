use std::{env, process::exit};

use librespot::{
    core::{
        authentication::Credentials, config::SessionConfig, session::Session,
        spotify_uri::SpotifyUri,
    },
    metadata::{Metadata, Album, Track},
};

#[tokio::main]
async fn main() {
    env_logger::init();
    let session_config = SessionConfig::default();

    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: {} ACCESS_TOKEN ALBUM", args[0]);
        return;
    }
    let credentials = Credentials::with_access_token(&args[1]);

    let album_uri = SpotifyUri::from_uri(&args[2]).unwrap_or_else(|_| {
        eprintln!(
            "ALBUM should be an album URI such as: \
                \"spotify:album:1ATL5GLyefJaxhQzSPVrLX\""
        );
        exit(1);
    });

    let session = Session::new(session_config, None);
    if let Err(e) = session.connect(credentials, false).await {
        println!("Error connecting: {e}");
        exit(1);
    }

    let album = Album::get(&session, &album_uri).await.unwrap();
    println!("{album:?}");
    for track_id in album.tracks() {
        let album_track = Track::get(&session, track_id).await.unwrap();
        println!("track: {} {}", album_track.name, track_id);
    }
}
