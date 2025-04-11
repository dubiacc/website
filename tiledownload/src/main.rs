use anyhow::{Context, Result};
use image::{ColorType, ImageEncoder};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::Client;
use std::{fs, sync::Arc, time::Duration};
use tokio::{fs::File, io::AsyncWriteExt, sync::Semaphore};

const BASE_URL: &str = "https://tile.openstreetmap.org";
const OUTPUT_DIR: &str = "tiles";
const MAX_ZOOM: u32 = 7;
const MAX_CONCURRENT_DOWNLOADS: usize = 100;

#[tokio::main]
async fn main() -> Result<()> {
    // Create a client with appropriate headers and settings
    let client = Client::builder()
        .user_agent("OSM Tile Downloader (your@email.com)")
        .timeout(Duration::from_secs(30))
        .build()?;
    
    // Create a semaphore to limit concurrent downloads
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_DOWNLOADS));
    
    // Create progress bars
    let mp = MultiProgress::new();
    let total_tiles = (0..=MAX_ZOOM).map(|z| 4_u64.pow(z)).sum::<u64>();
    let total_progress = mp.add(ProgressBar::new(total_tiles));
    total_progress.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {bar:40} {pos}/{len} {percent}% {eta} {msg}")
            .unwrap(),
    );
    total_progress.set_message("Downloading tiles");
    
    // Create output directory
    fs::create_dir_all(OUTPUT_DIR).context("Failed to create output directory")?;
    
    let mut handles = Vec::new();
    
    // Iterate through each zoom level
    for z in 0..=MAX_ZOOM {
        let max_coord = 2_u32.pow(z);
        
        for x in 0..max_coord {
            for y in 0..max_coord {
                let client = client.clone();
                let semaphore = semaphore.clone();
                let total_progress = total_progress.clone();
                
                let handle = tokio::spawn(async move {
                    let permit = semaphore.acquire().await.unwrap();
                    let result = download_and_convert_tile(&client, z, x, y).await;
                    total_progress.inc(1);
                    drop(permit);
                    result
                });
                
                handles.push(handle);
            }
        }
    }
    
    // Wait for all downloads to complete
    for handle in handles {
        if let Err(e) = handle.await? {
            eprintln!("Error downloading tile: {}", e);
        }
    }
    
    total_progress.finish_with_message("All tiles downloaded and converted");
    
    Ok(())
}

async fn download_and_convert_tile(client: &Client, z: u32, x: u32, y: u32) -> Result<()> {
    // Create directory structure
    let dir_path = format!("{}/{}/{}", OUTPUT_DIR, z, x);
    fs::create_dir_all(&dir_path).context("Failed to create tile directory")?;
    
    // Download tile
    let url = format!("{}/{}/{}/{}.png", BASE_URL, z, x, y);
    let response = client.get(&url).send().await?;
    
    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "Failed to download tile {}/{}/{}: HTTP {}",
            z, x, y, response.status()
        ));
    }
    
    let png_data = response.bytes().await?;
    
    // Decode PNG
    let img = image::load_from_memory(&png_data)?;
    
    // Convert to AVIF
    let output_path = format!("{}/{}/{}/{}.avif", OUTPUT_DIR, z, x, y);
    let mut output_file = File::create(&output_path).await?;
    
    // Use a thread pool for CPU-intensive image conversion
    let avif_data = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
        let mut avif_data = Vec::new();
        let encoder = image::codecs::avif::AvifEncoder::new(&mut avif_data);
        encoder.write_image(
            img.as_bytes(),
            img.width(),
            img.height(),
            match img.color() {
                ColorType::L8 => ColorType::L8,
                ColorType::La8 => ColorType::La8,
                ColorType::Rgb8 => ColorType::Rgb8,
                ColorType::Rgba8 => ColorType::Rgba8,
                _ => ColorType::Rgba8,
            }.into(),
        )?;
        Ok(avif_data)
    }).await??;
    
    output_file.write_all(&avif_data).await?;
    
    Ok(())
}