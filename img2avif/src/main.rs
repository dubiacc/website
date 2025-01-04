use std::path::Path;

fn main() -> Result<(), String> {

    let mut cwd = std::env::current_dir()
        .map_err(|e| e.to_string())?;
    
    while !cwd.join("static").is_dir() {
        cwd = cwd.parent().ok_or("cannot find /static dir in current path")?.to_path_buf();
    }

    let dirs = vec![
        cwd.join("static"),
        cwd.join("articles"),
    ];

    for dir in dirs.iter() {
        walkdir::WalkDir::new(dir)
        .into_iter()
        .for_each(|entry| {
            if let Ok(entry) = entry {
                let entry = entry.path();
                let filename = entry.file_name().and_then(|s| s.to_str()).unwrap_or("");
                let allowed = ["png", "jpeg", "jpg", "webp", "bmp"];
                if !filename.ends_with("avif") && !filename.contains("logo-sm") && allowed.iter().any(|s| filename.ends_with(s)) {
                    let _ = image2avif(&entry);
                }
            }
        });
    }

    Ok(())
}

fn image2avif(image_path: &Path) -> Result<(), String> {

    let mut image_path_path = image_path.to_path_buf();
    image_path_path.set_extension("avif");

    let file = std::fs::read(&image_path)
    .map_err(|e| format!("cannot find image {:?}: {e}", image_path.display()))?;

    let transcoded = transcode_image_to_avif(&file)
    .map_err(|e| format!("cannot transcode image {:?}: {e}", image_path.display()))?;

    std::fs::write(&image_path_path, transcoded)
    .map_err(|e| format!("cannot write transcoded image {:?}: {e}", image_path.display()))?;

    let _ = std::fs::remove_file(&image_path);
    
    Ok(())
}

fn transcode_image_to_avif(bytes: &[u8]) -> Result<Vec<u8>, String> {
    use std::io::Cursor;
    let im = image::load_from_memory(bytes).map_err(|e| e.to_string())?;
    let im = im.resize(1024, 1024, image::imageops::FilterType::Triangle);
    let mut target = Cursor::new(Vec::<u8>::new());
    let _ = im.write_to(&mut target, image::ImageFormat::Avif).map_err(|e| e.to_string())?;
    Ok(target.into_inner())
}
