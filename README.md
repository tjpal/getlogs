# getlog
A simple command-line tool to fetch attachments from Jira issues and extract log files.

## Features
* Download attachments from specified Jira issues.
* Extract and save log files matching a configurable pattern.
* Organize outputs into dedicated directories per issue.

## Requirements
* Rust and Cargo installed (for building from source).

## Installation
Build the release binary:

```bash
cargo build --release
```

Optionally, you can install it to your system PATH:

```bash
# On Unix-like systems (Linux and macOS)
install --strip target/release/getlogs /usr/local/bin/
```

## Usage
All commands operate on one or more Jira issue keys.

### Fetch attachments
Downloads all attachments for the specified issues:

```bash
getlogs fetch ISSUE-1 ISSUE-2 ...
```

### Extract logs
Extracts log files (matching the configured pattern) into a subdirectory named `extracted-logs` within each issue folder:

```bash
getlogs extract ISSUE-1 ISSUE-2 ...
```

### Run all steps
Performs both fetch and extract operations for each issue:

```bash
getlogs all ISSUE-1 ISSUE-2 ...
```

## Configuration
By default, `getlogs` creates and uses `~/.getlog/config.json` on first run. The configuration file supports the following fields:

```json
{
  "default_path": "~/getlogs-data",         // Base directory for downloaded data
  "jira_url": "https://your-jira-instance", // Base URL of your Jira server
  "bearer_token": "<token>",                // Preferred authentication method
  "user_email": "<email>",                  // Used if bearer_token is absent
  "api_token": "<api_token>",               // Used if bearer_token is absent
  "logfile_regex": "\\.log$",               // Pattern to identify log files
  "archive_regex": "\\.log$"                // Pattern to identify log files within archives (optional)
}
```

* **default\_path**: Base directory where issue-specific folders are created.
* **jira\_url**: URL of your Jira instance (e.g., `https://jira.example.com`).
* **bearer\_token**: JWT or API token for authentication (preferred if provided).
* **user\_email** and **api\_token**: Jira credentials; used only if `bearer_token` is not set.
* **logfile\_regex**: Regular expression to match log file names (used during extraction).
* **archive\_regex**: Regular expression to match log file names *within an archive* (used during extraction). This is in particular useful if archived log files follow another naming pattern.

## Examples
```bash
# Fetch and extract logs for two issues
getlogs all PROJECT-12345 PROJECT-23456
```

After running, you'll find downloaded attachments under `~/getlogs-data/PROJECT-12345/` and extracted logs under `~/getlogs-data/PROJECT-12345/extracted-logs/`.

## License
MIT License. See [LICENSE](LICENSE) for details.