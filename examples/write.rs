use std::{env, fs::File, io::{self, SeekFrom, Read, Seek}, process::exit};

use librespot::{
    core::{authentication::Credentials, config::SessionConfig, session::Session, spotify_id::SpotifyId, SpotifyUri},
    audio::{AudioDecrypt, AudioFile},
    metadata::audio::{AudioFiles, AudioFileFormat, AudioItem},
};

#[tokio::main]
async fn main() {
    // CLI: ACCESS_TOKEN TRACK_BASE62_ID
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: {} ACCESS_TOKEN TRACK_BASE62_ID", args[0]);
        exit(2);
    }

    let access_token = &args[1];
    let track_base62 = &args[2];

    let credentials = Credentials::with_access_token(access_token);

    let session_config = SessionConfig::default();
    let session = Session::new(session_config, None);

    println!("Connecting...");
    if let Err(e) = session.connect(credentials, false).await {
        eprintln!("Error connecting: {e}");
        exit(1);
    }

    let track_uri = SpotifyUri::Track {
        id: SpotifyId::from_base62(track_base62).unwrap_or_else(|_| {
            eprintln!("Invalid track id");
            exit(1);
        }),
    };

    // Convert to SpotifyId (needed for audio key request later)
    let track_id: SpotifyId = match (&track_uri).try_into() {
        Ok(id) => id,
        Err(_) => {
            eprintln!("Could not convert URI to SpotifyId");
            exit(1);
        }
    };

    println!("Fetching audio item metadata...");
    let audio_item = match AudioItem::get_file(&session, track_uri.clone()).await {
        Ok(item) => item,
        Err(e) => {
            eprintln!("Unable to load audio item: {e:?}");
            exit(1);
        }
    };

    for (fmt, fid) in audio_item.files.iter() {
        println!("Available format: {:?} file_id: {}", fmt, fid);
    }
    // Pick first available format/file_id (example keeps it simple)
    let (format, file_id) = match audio_item.files.iter().next() {
        Some((fmt, fid)) => (*fmt, *fid),
        None => {
            eprintln!("No audio files available for this track");
            exit(1);
        }
    };

    // Decide extension based on format
    let out_ext = if AudioFiles::is_ogg_vorbis(format) {
        "ogg"
    } else if AudioFiles::is_mp3(format) {
        "mp3"
    } else if AudioFiles::is_flac(format) {
        "flac"
    } else {
        "bin"
    };

    // A lightweight mapping for bytes-per-second used during file open.
    // This mirrors the player's stream_data_rate approximations.
    let bytes_per_second: usize = match format {
        AudioFileFormat::OGG_VORBIS_96 | AudioFileFormat::MP3_96 => (12.0 * 1024.0) as usize,
        AudioFileFormat::OGG_VORBIS_160 | AudioFileFormat::MP3_160 | AudioFileFormat::MP3_160_ENC => (20.0 * 1024.0) as usize,
        AudioFileFormat::OGG_VORBIS_320 | AudioFileFormat::MP3_256 | AudioFileFormat::MP3_320 => (40.0 * 1024.0) as usize,
        AudioFileFormat::FLAC_FLAC => (112.0 * 1024.0) as usize,
        _ => (40.0 * 1024.0) as usize, // fallback
    };

    println!("Opening remote audio file (format: {:?})...", format);
    let encrypted_file_result = AudioFile::open(&session, file_id, bytes_per_second).await;
    let mut encrypted_file = match encrypted_file_result {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Unable to open audio file: {e:?}");
            exit(1);
        }
    };

    // Request audio key. If unavailable, we continue without decryption (some files are unencrypted).
    let key = match session.audio_key().request(track_id, file_id).await {
        Ok(k) => Some(k),
        Err(e) => {
            eprintln!("Warning: unable to obtain audio key, continuing without decryption: {e}");
            None
        }
    };

    // Wrap the remote file in the decrypting reader.
    let mut decrypted = AudioDecrypt::new(key, encrypted_file);

    // Build output filename
    let filename = format!("decrypted_{}.{}", track_id.to_base62().unwrap_or_default(), out_ext);

    println!("Writing decrypted container to {}", filename);

    // Perform a blocking copy of the decrypted bytes to disk. This writes the decrypted container
    // bytes (e.g., .ogg / .mp3 / .flac) — not decoded PCM.
    match File::create(&filename) {
        Ok(mut out_f) => {
            match io::copy(&mut decrypted, &mut out_f) {
                Ok(n) => {
                    println!("Wrote {} bytes to {}", n, filename);
                    // Seek back to start if needed by other code; not necessary here.
                    if let Err(e) = decrypted.seek(SeekFrom::Start(0)) {
                        eprintln!("Warning: failed to seek decrypted stream back to start: {e}");
                    }
                }
                Err(e) => {
                    eprintln!("Failed to write decrypted file: {e}");
                    exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("Unable to create {}: {e}", filename);
            exit(1);
        }
    }

    println!("Done.");
}
