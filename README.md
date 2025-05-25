<div align="center">

  <h3><a href="https://github.com/Whitecat18/vxug-scraper">VX-UNDERGROUND SCRAPER</a></h3>
  <img width="350px" src="./images/banner.png" alt="VX-Underground Scraper Logo" />
  <br><br>
  <p>A fast and efficient web scraper designed to download collections from <a href= "https://vx-underground.org" > vx-underground.org</a></p>
  <p>By <a href="https://x.com/5mukx">@5mukx</a></p>

  <img src="https://img.shields.io/badge/Language-Rust-orange" alt="Language: Rust" />
  <img src="https://img.shields.io/badge/OS-Windows-blue" alt="OS: Windows" />
  <img src="https://img.shields.io/badge/OS-Linux-white" alt="OS: Linux" />
  <img src="https://img.shields.io/badge/Maintained-Yes-green" alt="Maintained: Yes" />
  
</div>

-------
<br>

This tool recursively crawls, identifies downloadable files (.pdf, .zip, .7z), and saves them to a Download file.

### Features:-

Recursive Scraping: Automatically crawls subdirectories on vx-underground.org to find and download files.

* File Type Support: Downloads .pdf, .zip, and .7z files.
* Customizable Starting Point: Optionally specify a subdirectory to scrape a specific collection.
* Robust Error Handling: Retries failed downloads up to three times and skips invalid links.
* Organized Storage: Saves files in a directory structure mirroring the website's hierarchy.
* Visited URL Tracking: Prevents redundant scraping of already processed directories.
* User-Friendly Interface: Displays a banner and progress updates during scraping.

### Prerequisites

Rust: Ensure you have Rust installed. You can install it via rustup.


### Installation

Clone or download the repository containing the code.

Navigate to the project directory:

```bash
cd vxug-scraper
```

Build the project:

```bash
cargo build --release
```

To run the project: 

```bash
cargo run --release
```

### USAGE:

 > ⚠️ Use Tool with Caution. You may get banned temporary if you misuse this tool !
 
 You can optionally specify an output directory with `-o` or `--output-dir`. If not provided, defaults to `Downloads`.
 
 Example: Download the "Papers" collection to a custom directory:
 
```bash
cargo run --release -- -o /path/to/output Papers
```

You can also limit the time between requests using `-r` or `--rate-limit`. For example, wait 5 seconds between each HTTP call:

```bash
cargo run --release -- -r 5
```

Or combine with output-dir and collection:

```bash
cargo run --release -- -r 5 -o /path/to/output Papers
```


To scrape all the collections. 

```bash
cargo run --release
```

To download a particular collection !

```bash
cargo run -- <Directory of Page>
```

Example: 

![vx-ug](./images/image.png)

I'm choosing to download Papers so 

```bash
cargo run --release -- Papers
```

or if you want to download specfic directories for example: I need to download Windows Papers from Paper collections.

![vx-ug-papers](./images/image-1.png)

```bash 
cargo run --release -- Papers/Windows
```

If the path is contains space. Add %20 instead of space..

```bash
cargo run --release -- Papers/Malware%20Defense
```

### Sample Output

![Demo-1](./images/image-2.png)

Used: `cargo run --release -- Papers/Malware%20Defense`


-----

Contributors are welcome =) 

Check out my other works here: [Rust for Malware development Repository](https://github.com/Whitecat18/Rust-for-Malware-Development.git) 
