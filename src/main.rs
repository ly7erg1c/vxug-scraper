/*
    Web Scraper Code.
    @5mukx
*/

use futures::stream::{FuturesUnordered, StreamExt};
use regex::Regex;
use reqwest::Client;
use scraper::{Html, Selector};
use std::path::Path;
use std::collections::HashSet;
use std::error::Error;
use std::fs::{self, File};
use std::io::Write;
use std::sync::Arc;
use tokio::time::{Duration, sleep};
use std::sync::atomic::{AtomicU64, Ordering};

/// Global rate limit in seconds between HTTP requests (0 = no limit)
static RATE_LIMIT_SECS: AtomicU64 = AtomicU64::new(0);
/// Pause execution waiting for network switching and resume after countdown
fn pause_for_proxy_change() {
    eprintln!("[!] Rate limit or network error detected.");
    eprintln!("[*] Press Enter to start retry countdown...");
    let mut _enter = String::new();
    std::io::stdin().read_line(&mut _enter).unwrap();
    eprintln!("[*] Waiting for network switching to take effect...");
    for remaining in (1..=10).rev() {
        eprint!("\r[*] Retrying in {} seconds...", remaining);
        std::io::stdout().flush().unwrap();
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    eprintln!("\r[*] Resuming now...");
}


// async fn => 

#[tokio::main]
async fn main() -> Result<(), reqwest::Error> {
    let args: Vec<String> = std::env::args().collect();

    let base_url = "https://vx-underground.org";
    let mut start_path: Option<String> = None;
    let mut root_dir = String::from("Downloads");

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-o" | "--output-dir" => {
                if i + 1 >= args.len() {
                    eprintln!("Error: {} requires a value", args[i]);
                    std::process::exit(1);
                }
                root_dir = args[i + 1].clone();
                i += 2;
            }
            "-r" | "--rate-limit" => {
                if i + 1 >= args.len() {
                    eprintln!("Error: {} requires a value", args[i]);
                    std::process::exit(1);
                }
                let secs = args[i + 1].parse::<u64>().unwrap_or_else(|_| {
                    eprintln!("Error: invalid rate-limit value: {}", args[i + 1]);
                    std::process::exit(1);
                });
                RATE_LIMIT_SECS.store(secs, Ordering::Relaxed);
                i += 2;
            }
            other => {
                if start_path.is_some() {
                    eprintln!("Warning: multiple paths specified, using first: {}", other);
                } else {
                    start_path = Some(other.to_string());
                }
                i += 1;
            }
        }
    }

    banner();

    fs::create_dir_all(&root_dir).expect("Unable to create root directory");

    let mut start_url = format!("{}/", base_url);
    if let Some(path) = &start_path {
        start_url = format!("{}/{}", base_url, path);
        match check_url(&start_url) {
            Ok(true) => println!("[+] URL is reachable: {} - STATUS {}", start_url, 200),
            Ok(false) => eprintln!("[!] URL not reachable: {}", start_url),
            Err(e) => eprintln!("Error while checking URL: {}", e),
        }
        println!("[+] Downloading collection: {}", path);
    } else {
        println!("[+] No parameter detected. Starting to download all collections...");
    }

    println!("[*] Press Enter to Start Processing =>");

    std::io::stdin().read_line(&mut String::new()).unwrap();
    std::io::stdout().flush().unwrap();

    let client = Arc::new(Client::builder().build()?);
    println!("Starting scrape at URL: {}", start_url);

    let mut visited = HashSet::new();
    let skip_segments: Vec<String> = if let Some(path) = &start_path {
        path.split('/').map(|s| s.to_string()).collect()
    } else {
        Vec::new()
    };
    scrape_directory(client, base_url, &start_url, &root_dir, &mut visited, &skip_segments).await?;

    println!("Scraping and downloading complete!");
    Ok(())
}

async fn scrape_directory(
    client: Arc<Client>,
    base_url: &str,
    url: &str,
    dir: &str,
    visited: &mut HashSet<String>,
    skip_segments: &[String],
) -> Result<(), reqwest::Error> {
    println!("Processing URL: {} | Saving to directory: {}", url, dir);

    let current_dir = url
        .split('/')
        .filter(|s| !s.is_empty())
        .last()
        .unwrap_or("")
        .to_string();

    visited.insert(current_dir.clone());
    println!("Visited directories: {:?}", visited);

    // fetch page with rate-limit and retry up to 3 times on rate-limit or network errors
    let mut attempts = 0;
    let max_attempts = 3;
    let response_text = loop {
        attempts += 1;
        let rl = RATE_LIMIT_SECS.load(Ordering::Relaxed);
        if rl > 0 {
            sleep(Duration::from_secs(rl)).await;
        }
        match client.get(url).send().await {
            Ok(resp) => {
                let status = resp.status();
                if !status.is_success() {
                    eprintln!("[!] Attempt {}/{}: HTTP status {} received from {}.", attempts, max_attempts, status, url);
                } else {
                    match resp.text().await {
                        Ok(body) => break body,
                        Err(e) => {
                            eprintln!("[!] Attempt {}/{}: Error reading body from {}: {}.", attempts, max_attempts, url, e);
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("[!] Attempt {}/{}: HTTP request to {} failed: {}.", attempts, max_attempts, url, e);
            }
        }
        if attempts >= max_attempts {
            eprintln!("[!] Failed to fetch {} after {} attempts. Skipping...", url, max_attempts);
            return Ok(());
        }
        pause_for_proxy_change();
    };
    let document = Html::parse_document(&response_text);

    // check for .pdf or .zip files
    let link_selector =
        Selector::parse(r#"a[href$=".pdf"], a[href$=".zip"], a[href$=".7z"]"#).unwrap();
    let links: Vec<(String, String)> = document
        .select(&link_selector)
        .filter_map(|link| {
            let href = link.value().attr("href")?.to_string();
            let name = href.split('/').last()?.to_string();
            let sanitized_name = Regex::new(r"[<>:/\\|?*]")
                .unwrap()
                .replace_all(&name, "_")
                .to_string();
            Some((sanitized_name, href))
        })
        .collect();

    if !links.is_empty() {
        println!(
            "Found {} files at {}: {:?}",
            links.len(),
            url,
            links.iter().map(|(name, _)| name).collect::<Vec<_>>()
        );
        let mut download_tasks = FuturesUnordered::new();
        for (name, href) in links {
            let client = Arc::clone(&client);
            let file_path = format!("{}/{}", dir, name);
            if Path::new(&file_path).exists() {
                println!("Skipping {}: already exists at {}", name, file_path);
                continue;
            }
            let file_url = if href.starts_with("http") {
                href
            } else {
                format!("{}{}", base_url, href)
            };

            download_tasks.push(tokio::spawn(async move {
                println!("Downloading {} to {}", name, file_path);
                let mut success = false;
                for attempt in 1..=3 {
                    match download_file(&client, &file_url, &file_path).await {
                        Ok(_) => {
                            println!("Saved {} to {}", name, file_path);
                            success = true;
                            break;
                        }
                        Err(e) => {
                            eprintln!("Attempt {}/3 failed for {}: {}", attempt, name, e);
                            if attempt < 3 {
                                if attempt == 1 {
                                    pause_for_proxy_change();
                                } else {
                                    eprintln!("[*] Waiting for network switching to take effect...");
                                    for remaining in (1..=10).rev() {
                                        eprint!("\r[*] Retrying in {} seconds...", remaining);
                                        std::io::stdout().flush().unwrap();
                                        sleep(Duration::from_secs(1)).await;
                                    }
                                    eprintln!("\r[*] Resuming now...");
                                }
                            }
                        }
                    }
                }
                if !success {
                    eprintln!("[!] Failed to download {} after 3 attempts, skipping.", name);
                }
            }));
        }
        while let Some(res) = download_tasks.next().await {
            if let Err(e) = res {
                eprintln!("Download task failed in {}: {}", dir, e);
            }
        }
    } else {

        let category_selector =
            Selector::parse(r#"div.cursor-pointer span.text-white.text-xs.truncate"#).unwrap();
        let mut categories: Vec<String> = document
            .select(&category_selector)
            .map(|e| e.text().collect::<Vec<_>>().join("").trim().to_string())
            .collect();

        categories.retain(|category|
            !visited.contains(category)
            && category != &current_dir
            && !skip_segments.contains(category)
        );

        if categories.is_empty() {
            println!("No subdirectories found at {}", url);
            visited.remove(&current_dir);
            return Ok(());
        }

        println!("Found subdirectories at {}: {:?}", url, categories);
        for category in categories {
            let category_url = format!("{}/{}", url.trim_end_matches('/'), category);
            let category_dir = format!("{}/{}", dir, category);
            fs::create_dir_all(&category_dir).expect("Unable to create category directory");

            let recursive_call = Box::pin(scrape_directory(
                Arc::clone(&client),
                base_url,
                &category_url,
                &category_dir,
                visited,
                skip_segments,
            ));

            if let Err(e) = recursive_call.await {
                eprintln!("Failed to scrape {}: {}", category_url, e);
            }
        }
    }

    // remove the current dir ...
    visited.remove(&current_dir);
    Ok(())
}

async fn download_file(client: &Client, url: &str, file_path: &str) -> Result<(), reqwest::Error> {
    // single attempt fetch with rate-limit, return error on failure
    let rl = RATE_LIMIT_SECS.load(Ordering::Relaxed);
    if rl > 0 {
        sleep(Duration::from_secs(rl)).await;
    }
    let resp = client.get(url)
        .send()
        .await?
        .error_for_status()?;
    let bytes = resp.bytes().await?;

    if let Some(parent) = std::path::Path::new(file_path).parent() {
        fs::create_dir_all(parent).unwrap();
    }

    let mut file = File::create(file_path).unwrap();
    file.write_all(&bytes).unwrap();
    Ok(())
}

fn check_url(url: &str) -> Result<bool, Box<dyn Error>> {
    let response = reqwest::blocking::get(url)?;
    Ok(response.status().is_success())
}

fn banner() {
    println!("
██╗   ██╗██╗  ██╗      ██╗   ██╗ ██████╗     ███████╗ ██████╗██████╗  █████╗ ██████╗ ███████╗██████╗ 
██║   ██║╚██╗██╔╝      ██║   ██║██╔════╝     ██╔════╝██╔════╝██╔══██╗██╔══██╗██╔══██╗██╔════╝██╔══██╗
██║   ██║ ╚███╔╝ █████╗██║   ██║██║  ███╗    ███████╗██║     ██████╔╝███████║██████╔╝█████╗  ██████╔╝
╚██╗ ██╔╝ ██╔██╗ ╚════╝██║   ██║██║   ██║    ╚════██║██║     ██╔══██╗██╔══██║██╔═══╝ ██╔══╝  ██╔══██╗
 ╚████╔╝ ██╔╝ ██╗      ╚██████╔╝╚██████╔╝    ███████║╚██████╗██║  ██║██║  ██║██║     ███████╗██║  ██║
  ╚═══╝  ╚═╝  ╚═╝       ╚═════╝  ╚═════╝     ╚══════╝ ╚═════╝╚═╝  ╚═╝╚═╝  ╚═╝╚═╝     ╚══════╝╚═╝  ╚═╝

    An Fast Scraper that scraps vx-underground.org Collections....
                                                        Ft. @5mukx                                                                                
    ")
}
