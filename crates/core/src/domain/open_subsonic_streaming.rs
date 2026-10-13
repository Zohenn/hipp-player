use crate::data::{app_data_dir, log_error};
use crate::open_subsonic::OpenSubsonicClient;
use color_eyre::eyre::{Context, Result};
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;
use tokio_stream::StreamExt;

#[derive(Clone)]
pub struct OpenSubsonicStreamingService {
    client: OpenSubsonicClient,
    cache_dir: PathBuf,
}

impl OpenSubsonicStreamingService {
    pub fn new(client: OpenSubsonicClient) -> Result<Self> {
        let cache_dir = app_data_dir().join("cache").join("songs");
        std::fs::create_dir_all(&cache_dir).context("failed to create song cache directory")?;

        Ok(Self { client, cache_dir })
    }

    pub async fn stream_song(&mut self, song_id: &str) -> Result<std::fs::File> {
        let cache_path = self.cache_dir.join(song_id);
        if let Ok(cache_file) = std::fs::File::open(&cache_path) {
            return Ok(cache_file);
        }

        let part_path = cache_path.with_extension(".part");
        let mut writer = tokio::io::BufWriter::new(
            tokio::fs::File::create(&part_path)
                .await
                .context("failed to open song file for remote streaming")?,
        );
        let reader = std::fs::File::open(&part_path)?;

        let mut stream = self.client.stream_song(song_id).await?;

        const PREBUFFER: u64 = 256 * 1024;
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel::<()>();
        let mut ready_tx = Some(ready_tx);

        tokio::spawn(async move {
            let result: Result<()> = async {
                let mut written = 0u64;

                while let Some(chunk) = stream.next().await {
                    let chunk = chunk?;
                    writer.write_all(&chunk).await?;
                    written += chunk.len() as u64;

                    if written >= PREBUFFER {
                        if let Some(ready_tx) = ready_tx.take() {
                            writer.flush().await?;
                            let _ = ready_tx.send(());
                        }
                    }
                }
                writer.flush().await?;
                tokio::fs::rename(&part_path, &cache_path).await?;

                Ok(())
            }
            .await;

            if let Err(e) = result {
                log_error(format!("{e:?}"));
                let _ = tokio::fs::remove_file(&part_path).await;
            }
        });

        ready_rx
            .await
            .context("download failed before first chunk")?;

        // TODO: we should return some wrapper around file with Read + Seek that handles player reaching eof before streaming ends
        Ok(reader)
    }

    // todo: a method that allows starting a fetch for the next queued song, this will also need to store some map for pending fetches i guess
}
