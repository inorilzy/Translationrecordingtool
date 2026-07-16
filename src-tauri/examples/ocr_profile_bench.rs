use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    time::Instant,
};

use image::GenericImageView;
use ppocr_rs::{OcrLite, OcrOptions};

fn main() {
    if let Err(error) = run() {
        eprintln!("OCR bench failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let workspace = workspace_root()?;
    let ort = workspace
        .join("src-tauri")
        .join("binaries")
        .join("onnxruntime.dll");
    if !ort.is_file() {
        return Err(format!("missing ORT dll: {}", ort.display()));
    }
    env::set_var("ORT_DYLIB_PATH", &ort);

    let sample = workspace.join("docs").join("assets").join("product-preview.png");
    if !sample.is_file() {
        return Err(format!("missing sample image: {}", sample.display()));
    }

    let image = image::open(&sample)
        .map_err(|e| format!("open image: {e}"))?
        .to_rgb8();
    let (width, height) = image.dimensions();
    println!(
        "sample={} size={}x{} bytes={}",
        sample.display(),
        width,
        height,
        fs::metadata(&sample).map(|m| m.len()).unwrap_or(0)
    );

    let cache_root = env::temp_dir().join("translate-tool-ocr-bench");
    fs::create_dir_all(&cache_root).map_err(|e| e.to_string())?;

    for profile in ["small", "medium"] {
        println!("\n===== profile={profile} =====");
        let model_dir = workspace
            .join("src-tauri")
            .join("resources")
            .join("ocr-models")
            .join(profile);
        let det = model_dir.join("det").join("inference.onnx");
        let rec = model_dir.join("rec").join("inference.onnx");
        let yml = model_dir.join("rec").join("inference.yml");
        for path in [&det, &rec, &yml] {
            if !path.is_file() {
                return Err(format!("missing model file: {}", path.display()));
            }
        }

        let det_mb = file_mb(&det)?;
        let rec_mb = file_mb(&rec)?;
        let yml_mb = file_mb(&yml)?;
        let total_mb = dir_mb(&model_dir)?;
        println!(
            "files: det={det_mb:.2}MB rec={rec_mb:.2}MB yml={yml_mb:.2}MB total_dir={total_mb:.2}MB"
        );

        let dict = cache_root.join(format!("{profile}-dict.txt"));
        let dict_start = Instant::now();
        extract_dict_from_yml(&yml, &dict)?;
        let dict_ms = dict_start.elapsed().as_millis();
        println!("dict_extract_ms={dict_ms} dict_chars={}", count_lines(&dict)?);

        let mut engine = OcrLite::new();
        let init_start = Instant::now();
        engine
            .init_models_no_angle(
                path_str(&det)?,
                path_str(&rec)?,
                path_str(&dict)?,
                4,
            )
            .map_err(|e| format!("init {profile}: {e}"))?;
        let init_ms = init_start.elapsed().as_millis();
        println!("init_ms={init_ms}");

        // Warmup once so first-call JIT/load noise is separated.
        let _ = engine
            .detect_with_options(
                &image,
                10,
                960,
                0.6,
                0.3,
                1.6,
                false,
                false,
                OcrOptions {
                    use_doc_orientation: false,
                    ..OcrOptions::default()
                },
            )
            .map_err(|e| format!("warmup {profile}: {e}"))?;

        let mut times = Vec::new();
        let mut last_text = String::new();
        let mut last_blocks = 0usize;
        for i in 0..5 {
            let start = Instant::now();
            let result = engine
                .detect_with_options(
                    &image,
                    10,
                    960,
                    0.6,
                    0.3,
                    1.6,
                    false,
                    false,
                    OcrOptions {
                        use_doc_orientation: false,
                        ..OcrOptions::default()
                    },
                )
                .map_err(|e| format!("detect {profile} run {i}: {e}"))?;
            let ms = start.elapsed().as_millis();
            times.push(ms);
            last_blocks = result.text_blocks.len();
            last_text = result
                .text_blocks
                .into_iter()
                .filter(|b| !b.text.trim().is_empty())
                .map(|b| b.text.trim().to_string())
                .collect::<Vec<_>>()
                .join("\n");
            println!("run{i}_ms={ms} blocks={last_blocks}");
        }

        let avg = times.iter().sum::<u128>() as f64 / times.len() as f64;
        let min = *times.iter().min().unwrap();
        let max = *times.iter().max().unwrap();
        println!("infer_avg_ms={avg:.1} min={min} max={max} blocks={last_blocks}");
        println!("--- recognized text begin ---");
        println!("{last_text}");
        println!("--- recognized text end ---");
        println!(
            "chars={} lines={}",
            last_text.chars().count(),
            last_text.lines().count()
        );
    }

    Ok(())
}

fn workspace_root() -> Result<PathBuf, String> {
    let current = env::current_dir().map_err(|e| e.to_string())?;
    current
        .ancestors()
        .find(|dir| {
            dir.join("package.json").is_file()
                && dir
                    .join("src-tauri")
                    .join("resources")
                    .join("ocr-models")
                    .is_dir()
        })
        .map(Path::to_path_buf)
        .ok_or_else(|| "workspace root not found".to_string())
}

fn extract_dict_from_yml(yml_path: &Path, dict_path: &Path) -> Result<(), String> {
    let content = fs::read_to_string(yml_path).map_err(|e| e.to_string())?;
    let mut chars = Vec::new();
    let mut in_dict = false;
    for line in content.lines() {
        if !in_dict {
            if line.trim_start().starts_with("character_dict:") {
                in_dict = true;
            }
            continue;
        }
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("- ") {
            let rest = rest.trim_end_matches('\r');
            let ch = if rest.starts_with('\'') && rest.ends_with('\'') && rest.len() >= 2 {
                rest[1..rest.len() - 1].replace("''", "'")
            } else {
                rest.to_string()
            };
            chars.push(ch);
        } else if !trimmed.is_empty() && !trimmed.starts_with('-') {
            break;
        }
    }
    if chars.is_empty() {
        return Err("empty character dict".to_string());
    }
    let mut file = fs::File::create(dict_path).map_err(|e| e.to_string())?;
    for ch in chars {
        writeln!(file, "{ch}").map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn file_mb(path: &Path) -> Result<f64, String> {
    Ok(fs::metadata(path).map_err(|e| e.to_string())?.len() as f64 / 1024.0 / 1024.0)
}

fn dir_mb(path: &Path) -> Result<f64, String> {
    let mut total = 0u64;
    for entry in walkdir(path)? {
        total += fs::metadata(entry).map_err(|e| e.to_string())?.len();
    }
    Ok(total as f64 / 1024.0 / 1024.0)
}

fn walkdir(path: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    fn rec(path: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
        for entry in fs::read_dir(path).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let p = entry.path();
            if p.is_dir() {
                rec(&p, files)?;
            } else {
                files.push(p);
            }
        }
        Ok(())
    }
    rec(path, &mut files)?;
    Ok(files)
}

fn count_lines(path: &Path) -> Result<usize, String> {
    Ok(fs::read_to_string(path)
        .map_err(|e| e.to_string())?
        .lines()
        .filter(|l| !l.is_empty())
        .count())
}

fn path_str(path: &Path) -> Result<&str, String> {
    path.to_str()
        .ok_or_else(|| format!("invalid path: {}", path.display()))
}
