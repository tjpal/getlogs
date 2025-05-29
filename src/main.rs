use std::{fs, path::PathBuf, io::{self, Cursor}};
use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;
use reqwest::{Client, Proxy, header::{HeaderMap, HeaderValue, AUTHORIZATION}};
use serde::{Deserialize, Serialize};
use zip::ZipArchive;
use futures_util::stream::StreamExt;

#[derive(Parser)]
#[clap(name = "getlogs", version = "1.0.0", author = "")]
struct Cli {
    #[clap(subcommand)]
    command: Command,

    #[clap(global = true)]
    jira_ids: Vec<String>,
}

#[derive(Subcommand)]
enum Command {
    Fetch,
    Extract,
    Convert,
    All,
}

/// Configuration stored in ~/.getlogs/config.json
#[derive(Serialize, Deserialize, Debug)]
struct Config {
    default_path: PathBuf,
    jira_url: String,
    proxy: Option<String>,
    bearer_token: Option<String>,
    user_email: Option<String>,
    api_token: Option<String>,
    logfile_regex: String,
    archive_regex: Option<String>
}

impl Config {
    fn load_or_create() -> io::Result<Self> {
        let home = dirs::home_dir().expect("Could not find home directory");

        let config_dir = home.join(".getlogs");
        if !config_dir.exists() {
            fs::create_dir_all(&config_dir)?;
        }

        let config_file = config_dir.join("config.json");
        if !config_file.exists() {
            let default = Config {
                default_path: home.join("logs"),
                jira_url: "https://your-jira-server.com".to_string(),
                proxy: None,
                bearer_token: None,
                user_email: None,
                api_token: None,
                logfile_regex: r".*\.(logcat|dlt|txt)$".to_string(),
                archive_regex: None
            };

            let contents = serde_json::to_string_pretty(&default)?;
            fs::write(&config_file, contents)?;

            eprintln!("Created default config at {}. Please update it with either `bearer_token` or `user_email` + `api_token`, then rerun.", config_file.display());
            std::process::exit(1);
        }

        let data = fs::read_to_string(&config_file)?;
        let config: Config = serde_json::from_str(&data)?;

        Ok(config)
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let config = Config::load_or_create()?;

    for jira_id in &cli.jira_ids {
        let base_path = PathBuf::from(&config.default_path).join(jira_id);
        fs::create_dir_all(&base_path)?;

        println!("=== {} ===", jira_id);

        if matches!(cli.command, Command::Fetch | Command::All) {
            fetch_attachments(&config, jira_id, &base_path).await?;
        }

        let extract_path = base_path.join("logs-extracted");
        if matches!(cli.command, Command::Extract | Command::All) {
            extract_logs(&base_path, &extract_path, &config)?;
        }

        if matches!(cli.command, Command::Convert | Command::All) {
            convert_logs(&extract_path)?;
        }
    }

    Ok(())
}

async fn auth_request(client: &Client, config: &Config, url: &str) -> anyhow::Result<reqwest::Response> {
    if let Some(token) = &config.bearer_token {
        let auth_val = format!("Bearer {}", token);
        let mut headers = HeaderMap::new();

        headers.insert(AUTHORIZATION, HeaderValue::from_str(&auth_val)?);

        Ok(client.get(url).headers(headers).send().await?)
    } else if let (Some(email), Some(api_token)) = (&config.user_email, &config.api_token) {
        Ok(client.get(url).basic_auth(email, Some(api_token)).send().await?)
    } else {
        anyhow::bail!("No authentication configured: set either bearer_token or user_email+api_token in config");
    }
}

fn create_http_client(config: &Config) -> Client {
    if let Some(proxy_url) = &config.proxy {
        Client::builder()
            .proxy(Proxy::all(proxy_url).expect("Could not resolve proxy URL"))
            .build().expect("Could not create HTTP client with specified proxy URL")
    } else {
        Client::new()
    }
}

async fn fetch_attachments(config: &Config, issue: &str, dest: &PathBuf) -> anyhow::Result<()> {
    let client = create_http_client(&config);

    let url = format!("{}/rest/api/2/issue/{}?fields=attachment", config.jira_url, issue);

    // Fetch the attachment field
    let response = auth_request(&client, config, &url).await?;
    let json: serde_json::Value = response.json().await?;

    if let Some(atts) = json["fields"]["attachment"].as_array() {
        // Fetch the attachments
        for att in atts {
            let fname = att["filename"].as_str().unwrap();
            let file_url = att["content"].as_str().unwrap();
            let out_path = dest.join(fname);

            let response = auth_request(&client, config, file_url).await?;
            let total = response.content_length().unwrap_or(0);

            let progress_bar = ProgressBar::new(total);
            let style = ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")?
                .progress_chars("=>-");
            progress_bar.set_style(style);

            let mut file = fs::File::create(&out_path)?;
            let mut stream = response.bytes_stream();

            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;
                progress_bar.inc(chunk.len() as u64);
                io::copy(&mut Cursor::new(chunk), &mut file)?;
            }

            progress_bar.finish_and_clear();
            println!("Downloaded {}", fname);
        }
    }

    Ok(())
}

fn extract_logs(src: &PathBuf, dest: &PathBuf, config: &Config) -> anyhow::Result<()> {
    fs::create_dir_all(dest)?;
    let logfile_regex = Regex::new(&config.logfile_regex).unwrap();
    let zipfile_regex = Regex::new(&config.archive_regex.as_deref().unwrap_or(&config.logfile_regex)).expect("No zip archive regex");

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            let fname = path.file_name().unwrap().to_string_lossy();

            if logfile_regex.is_match(&fname) {
                fs::copy(&path, dest.join(&*fname))?;
            } else if path.extension().map(|e| e == "zip").unwrap_or(false) {
                let file = fs::File::open(&path)?;
                let mut zip = ZipArchive::new(file)?;

                for i in 0..zip.len() {
                    let mut f = zip.by_index(i)?;
                    let name = f.name().to_string();

                    if zipfile_regex.is_match(&name) {
                        let out_path = dest.join(PathBuf::from(&name).file_name().unwrap());
                        let mut out = fs::File::create(&out_path)?;
                        io::copy(&mut f, &mut out)?;
                    }
                }
            }
        }
    }

    println!("Extraction complete to {}", dest.display());

    Ok(())
}

fn convert_logs(dir: &PathBuf) -> anyhow::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().map(|e| e == "dlt").unwrap_or(false) {
            // TODO. Pull out logcat out of dlt file.
        }
    }

    Ok(())
}
