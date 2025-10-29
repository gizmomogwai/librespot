use std::{env, fs::File, io::Write, process::exit, path::Path};

use librespot::{
    core::{
        authentication::Credentials,
        config::SessionConfig,
        file_id::FileId,
        session::Session,
        spotify_uri::SpotifyUri,
    },
    metadata::{Metadata, image::Image, Album, Track},
};

#[tokio::main]
async fn main() {
    env_logger::init();
    let session_config = SessionConfig::default();

    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: {} ACCESS_TOKEN ALBUM_URI_OR_ID", args[0]);
        eprintln!("Example: {} <ACCESS_TOKEN> spotify:album:1ATL5GLyefJaxhQzSPVrLX", args[0]);
        return;
    }
    let credentials = Credentials::with_access_token(&args[1]);

    // Accept spotify:album:<id>, open.spotify.com/album/<id>, or a raw id
    let album_uri = if let Ok(u) = SpotifyUri::from_uri(&args[2]) {
        u
    } else {
        // Try to build spotify:album:<id> from a plain id or URL last segment
        let input = &args[2];
        let id = if input.starts_with("spotify:album:") {
            input.split(':').last().unwrap().to_string()
        } else if input.contains("/album/") {
            input
                .trim_end_matches('/')
                .split('/')
                .last()
                .unwrap()
                .split('?')
                .next()
                .unwrap()
                .to_string()
        } else {
            input.to_string()
        };
        let uri = format!("spotify:album:{}", id);
        SpotifyUri::from_uri(&uri).unwrap_or_else(|_| {
            eprintln!("ALBUM should be an album URI such as: \"spotify:album:<id>\"");
            exit(1);
        })
    };

    // Create and connect a librespot Session
    let session = Session::new(session_config, None);
    if let Err(e) = session.connect(credentials, false).await {
        eprintln!("Error connecting: {e}");
        exit(1);
    }

    // Fetch album metadata
    let album = match Album::get(&session, &album_uri).await {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Failed to fetch album metadata: {e}");
            exit(1);
        }
    };

    println!(
        "Got album: {:?} - {}",
        album,
        album.artists.iter().map(|a| a.name.clone()).collect::<Vec<_>>().join(", ")
    );

    // Images are stored on the Album as `covers` / `cover_group` (Images).
    // Images deref to a Vec<FileId>, so we can index into them.
    let image = album.covers.iter().max_by_key(|a|a.width).unwrap();
    println!("image: {:?}", image);

    // Request the image bytes via the session's spclient.
    // `spclient().get_image` returns the raw bytes for the image in most librepsot versions.
    let image_bytes = match session.spclient().get_image(&image.id).await {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Failed to fetch image from spclient: {e}");
            exit(1);
        }
    };

    let filename = format!("album-cover.jpg");
    let path = Path::new(&filename);
    let mut f = File::create(path).unwrap_or_else(|e| {
        eprintln!("Failed to create file {}: {}", filename, e);
        exit(1);
    });

    // If the spclient returns a wrapper type instead of Vec<u8>, you might need to adjust this line.
    if let Err(e) = f.write_all(&image_bytes) {
        eprintln!("Failed to write image file: {e}");
        exit(1);
    }

    println!("Saved album cover to {}", filename);
}
