/*
    Web Scraper Code.
    @5mukx
*/

use futures::stream::{FuturesUnordered, StreamExt};
use regex::Regex;
use reqwest::Client;
use scraper::{Html, Selector};
use std::collections::HashSet;
use std::error::Error;
use std::fs::{self, File};
use std::io::Write;
use std::sync::Arc;
use tokio::time::{Duration, sleep};


// async fn => 

#[tokio::main]
async fn main() -> Result<(), reqwest::Error> {
    let args: Vec<String> = std::env::args().collect();

    let base_url = "https://vx-underground.org";
    let mut start_url = "https://vx-underground.org/".to_string();
    let root_dir = "Downloads";

    fs::create_dir_all(root_dir).expect("Unable to create root directory");

    banner();

    if args.len() > 1 {
        start_url = format!("https://vx-underground.org/{}", args[1]);

        match check_url(&start_url) {
            Ok(is_success) => {
                if is_success {
                    println!("[+] URL is reachable: {} - STATUS {}", start_url, 200);
                }
            }

            Err(e) => eprintln!("Error while checking URL: {}", e),
        }

        println!(
            "[+] Specfic Parameter Detected. Downloading {} Collections...",
            args[1]
        );

    } else {
        println!("[+] No Parameter Detected. Starting to Download All Collections ...")
    }

    println!("[*] Press Enter to Start Processing =>");

    let _ = std::io::stdin().read_line(&mut String::new()).unwrap();
    let _ = std::io::stdout().flush();

    let client = Arc::new(Client::builder().build()?);
    println!("Starting scrape at URL: {}", start_url);

    let mut visited = HashSet::new();
    scrape_directory(client, base_url, &start_url, root_dir, &mut visited).await?;

    println!("Scraping and downloading complete!");
    Ok(())
}

async fn scrape_directory(
    client: Arc<Client>,
    base_url: &str,
    url: &str,
    dir: &str,
    visited: &mut HashSet<String>,
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

    let response = client.get(url).send().await?.text().await?;
    let document = Html::parse_document(&response);

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
            let file_url = if href.starts_with("http") {
                href
            } else {
                format!("{}{}", base_url, href)
            };

            download_tasks.push(tokio::spawn(async move {
                println!("Downloading {} to {}", name, file_path);
                for attempt in 1..=3 {
                    match download_file(&client, &file_url, &file_path).await {
                        Ok(_) => {
                            println!("Saved {} to {}", name, file_path);
                            break;
                        }
                        Err(e) => {
                            eprintln!("Attempt {}/3 failed for {}: {}", attempt, name, e);
                            if attempt < 3 {
                                sleep(Duration::from_secs(1)).await;
                            }
                        }
                    }
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
            .map(|e| e.inner_html().trim().to_string())
            .collect();

        // filter out dirs, if they are visited !
        categories.retain(|category| !visited.contains(category) && category != &current_dir);

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
    let response = client.get(url).send().await?;
    let bytes = response.bytes().await?;

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
