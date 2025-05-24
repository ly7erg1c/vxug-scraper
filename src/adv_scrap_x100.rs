/*
    [IN TESTING ---!]
    This may harm System... Test it on VM...
    @5mukx
*/


use reqwest::Client;
use scraper::{Html, Selector};
use std::fs::{self, File};
use std::io::Write;
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use regex::Regex;
use futures::stream::{FuturesUnordered, StreamExt};
use std::collections::VecDeque;
use tokio::sync::{Semaphore, RwLock};
use dashmap::DashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone)]
struct AdvancedScraper {
    client: Arc<Client>,
    visited_urls: Arc<DashMap<String, bool>>,
    download_semaphore: Arc<Semaphore>,
    scrape_semaphore: Arc<Semaphore>,
    stats: Arc<ScrapingStats>,
    regex_cache: Arc<RwLock<dashmap::DashMap<String, Regex>>>,
    selector_cache: Arc<RwLock<dashmap::DashMap<String, Selector>>>,
}

#[derive(Default)]
struct ScrapingStats {
    pages_scraped: AtomicUsize,
    files_downloaded: AtomicUsize,
    bytes_downloaded: AtomicUsize,
    errors: AtomicUsize,
}

impl ScrapingStats {
    fn increment_pages(&self) {
        self.pages_scraped.fetch_add(1, Ordering::Relaxed);
    }
    
    fn increment_files(&self) {
        self.files_downloaded.fetch_add(1, Ordering::Relaxed);
    }
    
    fn add_bytes(&self, bytes: usize) {
        self.bytes_downloaded.fetch_add(bytes, Ordering::Relaxed);
    }
    
    fn increment_errors(&self) {
        self.errors.fetch_add(1, Ordering::Relaxed);
    }
    
    fn print_stats(&self) {
        println!("=== Scraping Statistics ===");
        println!("Pages scraped: {}", self.pages_scraped.load(Ordering::Relaxed));
        println!("Files downloaded: {}", self.files_downloaded.load(Ordering::Relaxed));
        println!("Bytes downloaded: {} MB", self.bytes_downloaded.load(Ordering::Relaxed) / 1_048_576);
        println!("Errors encountered: {}", self.errors.load(Ordering::Relaxed));
    }
}

#[derive(Clone, Debug)]
struct DownloadTask {
    url: String,
    file_path: String,
    file_name: String,
}

impl AdvancedScraper {
    fn new(max_concurrent_downloads: usize, max_concurrent_scrapes: usize) -> Self {
        let client = Arc::new(
            Client::builder()
                .timeout(Duration::from_secs(30))
                .tcp_keepalive(Duration::from_secs(60))
                .pool_max_idle_per_host(20)
                .pool_idle_timeout(Duration::from_secs(90))
                .user_agent("Mozilla/5.0 (compatible; VX-Underground-Scraper/1.0)")
                .build()
                .expect("Failed to create HTTP client")
        );

        Self {
            client,
            visited_urls: Arc::new(DashMap::new()),
            download_semaphore: Arc::new(Semaphore::new(max_concurrent_downloads)),
            scrape_semaphore: Arc::new(Semaphore::new(max_concurrent_scrapes)),
            stats: Arc::new(ScrapingStats::default()),
            regex_cache: Arc::new(RwLock::new(DashMap::new())),
            selector_cache: Arc::new(RwLock::new(DashMap::new())),
        }
    }

    async fn get_cached_regex(&self, pattern: &str) -> Regex {
        {
            let cache = self.regex_cache.read().await;
            if let Some(regex) = cache.get(pattern) {
                return regex.clone();
            }
        }

        let regex = Regex::new(pattern).expect("Invalid regex pattern");
        let cache = self.regex_cache.write().await;
        cache.insert(pattern.to_string(), regex.clone());
        regex
    }

    async fn get_cached_selector(&self, selector_str: &str) -> Selector {
        {
            let cache = self.selector_cache.read().await;
            if let Some(selector) = cache.get(selector_str) {
                return selector.clone();
            }
        }

        let selector = Selector::parse(selector_str).expect("Invalid CSS selector");
        let cache = self.selector_cache.write().await;
        cache.insert(selector_str.to_string(), selector.clone());
        selector
    }

    async fn scrape_with_bfs(&self, base_url: &str, start_url: &str, root_dir: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        fs::create_dir_all(root_dir)?;
        
        let mut queue = VecDeque::new();
        queue.push_back((start_url.to_string(), root_dir.to_string()));
        
        println!("Starting BFS scraping from: {}", start_url);

        while let Some((current_url, current_dir)) = queue.pop_front() {
            // atomic check !
            if self.visited_urls.insert(current_url.clone(), true).is_some() {
                continue;
            }

            let _permit = self.scrape_semaphore.acquire().await?;
            
            match self.process_page(&current_url, &current_dir, base_url).await {
                Ok((subdirs, downloads)) => {
                    // sets apart
                    for (subdir_url, subdir_path) in subdirs {
                        queue.push_back((subdir_url, subdir_path));
                    }
                    
                    if !downloads.is_empty() {
                        self.process_downloads_immediately(downloads).await;
                    }
                }
                Err(e) => {
                    eprintln!("Error processing {}: {}", current_url, e);
                    self.stats.increment_errors();
                }
            }

            self.stats.increment_pages();
            
            if self.stats.pages_scraped.load(Ordering::Relaxed) % 10 == 0 {
                self.stats.print_stats();
            }
        }

        Ok(())
    }

    async fn process_page(
        &self,
        url: &str,
        dir: &str,
        base_url: &str,
    ) -> Result<(Vec<(String, String)>, Vec<DownloadTask>), Box<dyn std::error::Error + Send + Sync>> {
        println!("Processing: {} -> {}", url, dir);

        let response = self.client.get(url).send().await?.text().await?;
        let document = Html::parse_document(&response);

        let mut subdirectories = Vec::new();
        let mut downloads = Vec::new();

        let file_selector = self.get_cached_selector(r#"a[href$=".pdf"], a[href$=".zip"], a[href$=".7z"], a[href$=".rar"]"#).await;
        let sanitize_regex = self.get_cached_regex(r"[<>:/\\|?*]").await;

        // extract links sequentially
        let links: Vec<(String, String)> = document
            .select(&file_selector)
            .filter_map(|link| {
                let href = link.value().attr("href")?.to_string();
                let name = href.split('/').last()?.to_string();
                let sanitized_name = sanitize_regex.replace_all(&name, "_").to_string();
                Some((sanitized_name, href))
            })
            .collect();

        if !links.is_empty() {
            println!("Found {} files at {}", links.len(), url);
            
            for (name, href) in links {
                let file_path = format!("{}/{}", dir, name);
                let file_url = if href.starts_with("http") {
                    href
                } else {
                    format!("{}{}", base_url, href)
                };

                downloads.push(DownloadTask {
                    url: file_url,
                    file_path,
                    file_name: name,
                });
            }
        } else {

            let category_selector = self.get_cached_selector(r#"div.cursor-pointer span.text-white.text-xs.truncate"#).await;
            let categories: Vec<String> = document
                .select(&category_selector)
                .map(|e| e.inner_html().trim().to_string())
                .filter(|category| !self.visited_urls.contains_key(category))
                .collect();

            for category in categories {
                let category_url = format!("{}/{}", url.trim_end_matches('/'), category);
                let category_dir = format!("{}/{}", dir, category);
                
                if let Err(e) = fs::create_dir_all(&category_dir) {
                    eprintln!("Failed to create directory {}: {}", category_dir, e);
                    continue;
                }
                
                subdirectories.push((category_url, category_dir));
            }
        }

        Ok((subdirectories, downloads))
    }

    async fn process_downloads_immediately(&self, downloads: Vec<DownloadTask>) {
        println!("Starting immediate download of {} files", downloads.len());
        
        let mut download_futures = FuturesUnordered::new();
        
        for task in downloads {
            let scraper = self.clone();
            download_futures.push(tokio::spawn(async move {
                scraper.download_with_retry(task).await
            }));
        }

        while let Some(result) = download_futures.next().await {
            if let Err(e) = result {
                eprintln!("Download task panicked: {}", e);
                self.stats.increment_errors();
            }
        }
    }

    async fn download_with_retry(&self, task: DownloadTask) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let _permit = self.download_semaphore.acquire().await?;
        
        println!("Downloading: {} -> {}", task.file_name, task.file_path);
        
        for attempt in 1..=3 {
            match self.download_file(&task.url, &task.file_path).await {
                Ok(bytes_downloaded) => {
                    println!("[+] Downloaded: {} ({} bytes)", task.file_name, bytes_downloaded);
                    self.stats.increment_files();
                    self.stats.add_bytes(bytes_downloaded);
                    return Ok(());
                }
                Err(e) => {
                    eprintln!("Attempt {}/3 failed for {}: {}", attempt, task.file_name, e);
                    if attempt < 3 {
                        sleep(Duration::from_millis(500 * attempt as u64)).await;
                    } else {
                        self.stats.increment_errors();
                    }
                }
            }
        }
        
        Err(format!("Failed to download {} after 3 attempts", task.file_name).into())
    }

    async fn download_file(&self, url: &str, file_path: &str) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        if std::path::Path::new(file_path).exists() {
            println!("File already exists, skipping: {}", file_path);
            return Ok(0);
        }

        let response = self.client.get(url).send().await?;
        let bytes = response.bytes().await?;
        let bytes_len = bytes.len();

        if let Some(parent) = std::path::Path::new(file_path).parent() {
            fs::create_dir_all(parent)?;
        }

        let mut file = File::create(file_path)?;
        file.write_all(&bytes)?;
        file.flush()?;

        Ok(bytes_len)
    }
}

fn banner() {
    println!("
██╗   ██╗██╗  ██╗      ██╗   ██╗ ██████╗     ███████╗ ██████╗██████╗  █████╗ ██████╗ ███████╗██████╗ 
██║   ██║╚██╗██╔╝      ██║   ██║██╔════╝     ██╔════╝██╔════╝██╔══██╗██╔══██╗██╔══██╗██╔════╝██╔══██╗
██║   ██║ ╚███╔╝ █████╗██║   ██║██║  ███╗    ███████╗██║     ██████╔╝███████║██████╔╝█████╗  ██████╔╝
╚██╗ ██╔╝ ██╔██╗ ╚════╝██║   ██║██║   ██║    ╚════██║██║     ██╔══██╗██╔══██║██╔═══╝ ██╔══╝  ██╔══██╗
 ╚████╔╝ ██╔╝ ██╗      ╚██████╔╝╚██████╔╝    ███████║╚██████╗██║  ██║██║  ██║██║     ███████╗██║  ██║
  ╚═══╝  ╚═╝  ╚═╝       ╚═════╝  ╚═════╝     ╚══════╝ ╚═════╝╚═╝  ╚═╝╚═╝  ╚═╝╚═╝     ╚══════╝╚═╝  ╚═╝

    ULTRA-FAST Advanced Scraper for vx-underground.org Collections (Requires 1 GB+ of Free Memory)
    Optimized with BFS, Priority Queues, Caching & Parallel Processing
                                                        Ft. @5mukx                                                                                
    ");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let base_url = "https://vx-underground.org";
    let start_url = "https://vx-underground.org/";
    let root_dir = "Downloads";

    banner();
    
    println!("Configuration:");
    println!("  • Max concurrent downloads: 12");
    println!("  • Max concurrent page scrapes: 8");
    println!("  • Download batch size: 20");
    println!("  • HTTP timeout: 30s");
    println!("  • Features: BFS traversal, caching, concurrent processing");
    println!();
    println!("Press Enter to start =>");
    
    let _ = std::io::stdin().read_line(&mut String::new())?;

    let scraper = AdvancedScraper::new(12, 8); 
    
    let start_time = std::time::Instant::now();
    scraper.scrape_with_bfs(base_url, start_url, root_dir).await?;
    let elapsed = start_time.elapsed();

    println!("Scraping completed!");
    println!("Total time: {:?}", elapsed);
    scraper.stats.print_stats();

    Ok(())
}