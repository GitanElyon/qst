use crate::app::App;
use dirs::config_dir;
use rustls::{ClientConfig, ClientConnection, Stream};
use std::{
    fs,
    io::{Read, Write},
    net::{TcpStream, ToSocketAddrs},
    path::{Component, Path, PathBuf},
    sync::Arc,
    time::Duration,
};

const CATALOG_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

pub struct CatalogSource {
    pub owner: &'static str,
    pub repo: &'static str,
    pub branch: &'static str,
}

impl Default for CatalogSource {
    fn default() -> Self {
        Self {
            owner: "GitanElyon",
            repo: "awesome-qst",
            branch: "main",
        }
    }
}

impl CatalogSource {
    fn raw_base(&self) -> String {
        format!(
            "https://raw.githubusercontent.com/{}/{}/{}",
            self.owner, self.repo, self.branch
        )
    }

    pub fn catalog_url(&self) -> String {
        format!("{}/catalog.tsv", self.raw_base())
    }

    pub fn script_url(&self, file: &str) -> String {
        format!("{}/scripts/{}", self.raw_base(), file)
    }
}

#[derive(Debug, Clone)]
pub struct CatalogEntry {
    pub file: String,
    pub name: String,
    pub version: String,
    #[allow(dead_code)]
    pub author: String,
    #[allow(dead_code)]
    pub description: String,
}

pub fn fetch_https_text(url: &str) -> Result<String, String> {
    rustls_graviola::default_provider().install_default().ok();

    let rest = url
        .strip_prefix("https://")
        .ok_or_else(|| format!("unsupported URL: {url}"))?;
    let (host, rest) = rest
        .split_once('/')
        .ok_or_else(|| format!("invalid URL: {url}"))?;
    let path = format!("/{rest}");

    let addr = (host, 443)
        .to_socket_addrs()
        .map_err(|err| err.to_string())?
        .next()
        .ok_or_else(|| format!("could not resolve host: {host}"))?;
    let mut tcp =
        TcpStream::connect_timeout(&addr, Duration::from_secs(5)).map_err(|err| err.to_string())?;
    tcp.set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|err| err.to_string())?;

    let mut root_store = rustls::RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = Arc::new(
        ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth(),
    );

    let server_name = rustls::pki_types::ServerName::try_from(host.to_string())
        .map_err(|_| format!("invalid host: {host}"))?;
    let mut tls_conn = ClientConnection::new(config, server_name).map_err(|err| err.to_string())?;
    let mut tls_stream = Stream::new(&mut tls_conn, &mut tcp);

    let request = format!("GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n");
    tls_stream
        .write_all(request.as_bytes())
        .map_err(|err| err.to_string())?;
    tls_stream.flush().map_err(|err| err.to_string())?;

    let mut response = Vec::new();
    tls_stream
        .read_to_end(&mut response)
        .map_err(|err| err.to_string())?;
    let response = String::from_utf8(response).map_err(|err| err.to_string())?;

    let status = response.lines().next().ok_or("empty response")?;
    let code = status
        .split_whitespace()
        .nth(1)
        .ok_or("malformed response")?;
    if !code.starts_with('2') {
        return Err(format!("server returned HTTP {code}"));
    }

    let body = response.split("\r\n\r\n").nth(1).unwrap_or_default();
    let body = body.trim().to_string();
    if body.is_empty() {
        Err("empty response body".to_string())
    } else {
        Ok(body)
    }
}

fn catalog_cache_path() -> Option<PathBuf> {
    let mut dir = config_dir()?;
    dir.push("qst");
    dir.push("storage");
    dir.push("catalog.tsv");
    Some(dir)
}

pub fn catalog_is_fresh() -> bool {
    let Some(path) = catalog_cache_path() else {
        return false;
    };
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    let Ok(modified) = metadata.modified() else {
        return false;
    };
    modified
        .elapsed()
        .map(|age| age < CATALOG_MAX_AGE)
        .unwrap_or(false)
}

pub fn refresh_catalog(force: bool) -> Result<(), String> {
    if !force && catalog_is_fresh() {
        return Ok(());
    }

    let body = fetch_https_text(&CatalogSource::default().catalog_url())?;
    if !body.contains('\t') {
        return Err("catalog response is not a TSV file".to_string());
    }

    let mut body = body;
    if !body.ends_with('\n') {
        body.push('\n');
    }

    let Some(path) = catalog_cache_path() else {
        return Err("could not locate configuration directory".to_string());
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }

    let tmp_path = path.with_extension(format!("tmp-{}", std::process::id()));
    fs::write(&tmp_path, &body).map_err(|err| err.to_string())?;
    fs::rename(&tmp_path, &path).map_err(|err| err.to_string())?;
    Ok(())
}

pub fn parse_catalog(content: &str) -> Vec<CatalogEntry> {
    content
        .lines()
        .filter_map(|line| {
            if !line.contains('\t') {
                return None;
            }
            let mut parts = line.splitn(5, '\t');
            let file = parts.next()?.trim();
            if file.is_empty() {
                return None;
            }
            Some(CatalogEntry {
                file: file.to_string(),
                name: parts.next().unwrap_or_default().trim().to_string(),
                version: parts.next().unwrap_or_default().trim().to_string(),
                author: parts.next().unwrap_or_default().trim().to_string(),
                description: parts.next().unwrap_or_default().trim().to_string(),
            })
        })
        .collect()
}

pub fn read_catalog() -> Vec<CatalogEntry> {
    let Some(path) = catalog_cache_path() else {
        return Vec::new();
    };
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };
    parse_catalog(&content)
}

pub fn find_catalog_entry(entries: &[CatalogEntry], query: &str) -> Result<CatalogEntry, String> {
    let query = query.trim();
    if query.is_empty() {
        return Err("empty script name".to_string());
    }

    let matches: Vec<&CatalogEntry> = entries
        .iter()
        .filter(|entry| {
            let stem = Path::new(&entry.file)
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or(&entry.file);
            entry.file.eq_ignore_ascii_case(query)
                || stem.eq_ignore_ascii_case(query)
                || entry.name.eq_ignore_ascii_case(query)
        })
        .collect();

    match matches.len() {
        0 => Err(format!("no script found matching '{query}'")),
        1 => Ok(matches[0].clone()),
        _ => Err(format!(
            "multiple scripts match '{query}': {} — be more specific",
            matches
                .iter()
                .map(|entry| entry.file.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

pub fn lookup_catalog(query: &str) -> Result<CatalogEntry, String> {
    let entries = read_catalog();
    if entries.is_empty() {
        return Err("catalog is empty — run qst --refresh-catalog first".to_string());
    }
    find_catalog_entry(&entries, query)
}

pub fn version_less_than(a: &str, b: &str) -> bool {
    if a.is_empty() || b.is_empty() || a == b {
        return false;
    }

    let a_parts: Vec<&str> = a.split('.').collect();
    let b_parts: Vec<&str> = b.split('.').collect();

    for index in 0..a_parts.len().max(b_parts.len()) {
        let part_a = a_parts.get(index).copied().unwrap_or("0");
        let part_b = b_parts.get(index).copied().unwrap_or("0");

        if let (Ok(num_a), Ok(num_b)) = (part_a.parse::<u64>(), part_b.parse::<u64>()) {
            if num_a != num_b {
                return num_a < num_b;
            }
        } else {
            match part_a.cmp(part_b) {
                std::cmp::Ordering::Less => return true,
                std::cmp::Ordering::Greater => return false,
                std::cmp::Ordering::Equal => {}
            }
        }
    }

    false
}

fn scripts_dir() -> Option<PathBuf> {
    let mut dir = config_dir()?;
    dir.push("qst");
    dir.push("scripts");
    Some(dir)
}

fn safe_script_path(file: &str) -> Result<PathBuf, String> {
    let path = Path::new(file);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::Prefix(_) | Component::RootDir
            )
        })
    {
        return Err(format!("invalid script path: {file}"));
    }
    Ok(path.to_path_buf())
}

pub fn install_script(entry: &CatalogEntry, source: &CatalogSource) -> Result<(), String> {
    let file_path = safe_script_path(&entry.file)?;
    let Some(dir) = scripts_dir() else {
        return Err("could not locate configuration directory".to_string());
    };

    let dest = dir.join(&file_path);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }

    let body = fetch_https_text(&source.script_url(&entry.file))?;
    fs::write(&dest, &body).map_err(|err| err.to_string())?;

    let mut permissions = fs::metadata(&dest)
        .map_err(|err| err.to_string())?
        .permissions();
    use std::os::unix::fs::PermissionsExt;
    permissions.set_mode(permissions.mode() | 0o111);
    fs::set_permissions(&dest, permissions).map_err(|err| err.to_string())?;

    Ok(())
}

fn collect_installed_files(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            collect_installed_files(&path, files);
        } else {
            files.push(path);
        }
    }
}

fn installed_script_files() -> Vec<String> {
    let Some(dir) = scripts_dir() else {
        return Vec::new();
    };
    let mut files = Vec::new();
    collect_installed_files(&dir, &mut files);
    files
        .iter()
        .filter_map(|path| path.strip_prefix(&dir).ok().and_then(|rel| rel.to_str()))
        .map(str::to_string)
        .collect()
}

pub fn installed_script_exists(file: &str) -> bool {
    let Some(dir) = scripts_dir() else {
        return false;
    };
    let Ok(file_path) = safe_script_path(file) else {
        return false;
    };
    dir.join(file_path).is_file()
}

pub fn resolve_installed_script(query: &str) -> Result<String, String> {
    let query = query.trim();
    if query.is_empty() {
        return Err("empty script name".to_string());
    }

    let installed = installed_script_files();
    let matches: Vec<String> = installed
        .iter()
        .filter(|file| {
            let stem = Path::new(file)
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or(file);
            file.eq_ignore_ascii_case(query) || stem.eq_ignore_ascii_case(query)
        })
        .cloned()
        .collect();

    match matches.len() {
        0 => {
            if let Ok(entry) = lookup_catalog(query)
                && installed_script_exists(&entry.file)
            {
                Ok(entry.file)
            } else {
                Err(format!("no installed script matches '{query}'"))
            }
        }
        1 => Ok(matches[0].clone()),
        _ => Err(format!(
            "multiple installed scripts match '{query}': {}",
            matches.join(", ")
        )),
    }
}

pub fn remove_script(file: &str) -> Result<(), String> {
    let file_path = safe_script_path(file)?;
    let Some(dir) = scripts_dir() else {
        return Err("could not locate configuration directory".to_string());
    };

    let dest = dir.join(&file_path);
    if dest.exists() {
        fs::remove_file(&dest).map_err(|err| err.to_string())?;
    }

    remove_alias_for_script(file)?;
    Ok(())
}

fn alias_file_path() -> Option<PathBuf> {
    let mut dir = config_dir()?;
    dir.push("qst");
    let alias_path = dir.join("alias.toml");
    if alias_path.exists() {
        Some(alias_path)
    } else {
        let legacy_path = dir.join("Alias.toml");
        legacy_path.exists().then_some(legacy_path)
    }
}

fn remove_alias_key(
    table: &mut toml::map::Map<String, toml::Value>,
    path: &str,
    target: &str,
) -> usize {
    let mut removed = 0;
    let mut tables_to_prune: Vec<String> = Vec::new();
    let keys: Vec<String> = table.keys().cloned().collect();

    for key in keys {
        let full_key = if path.is_empty() {
            key.clone()
        } else {
            format!("{path}.{key}")
        };

        if let Some(toml::Value::String(_)) = table.get(&key) {
            if App::normalize_alias_key(&full_key) == target {
                table.remove(&key);
                removed += 1;
            }
        } else if let Some(toml::Value::Table(child)) = table.get_mut(&key) {
            let child_removed = remove_alias_key(child, &full_key, target);
            removed += child_removed;
            if child_removed > 0 && child.is_empty() {
                tables_to_prune.push(key.clone());
            }
        }
    }

    for key in tables_to_prune {
        table.remove(&key);
    }

    removed
}

fn remove_alias_for_script(file: &str) -> Result<(), String> {
    let Some(path) = alias_file_path() else {
        return Ok(());
    };

    let target = Path::new(file)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(file)
        .to_string();

    let Ok(contents) = fs::read_to_string(&path) else {
        return Ok(());
    };
    let Ok(mut value) = toml::from_str::<toml::Value>(&contents) else {
        return Ok(());
    };

    let removed = if value.get("scripts").is_some() {
        match value.get_mut("scripts") {
            Some(toml::Value::Table(table)) => remove_alias_key(table, "", &target),
            _ => 0,
        }
    } else {
        match value.as_table_mut() {
            Some(table) => remove_alias_key(table, "", &target),
            None => 0,
        }
    };

    if removed > 0 {
        let serialized = toml::to_string_pretty(&value).map_err(|err| err.to_string())?;
        fs::write(&path, serialized).map_err(|err| err.to_string())?;
    }

    Ok(())
}

pub fn installed_version(file: &str) -> Option<String> {
    let file_path = safe_script_path(file).ok()?;
    let dir = scripts_dir()?;
    let metadata = App::read_script_metadata_from_source(&dir.join(file_path))?;
    metadata.version
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_less_than_compares_numeric_segments() {
        assert!(version_less_than("1.2.1", "1.12.1"));
        assert!(version_less_than("1.0.0", "1.0.1"));
        assert!(version_less_than("0.9", "1.0"));
        assert!(version_less_than("1.0.0", "1.0.0.1"));
        assert!(!version_less_than("1.12.1", "1.2.1"));
        assert!(!version_less_than("1.0.0", "1.0.0"));
        assert!(!version_less_than("1.0.0", "1.0.0.0"));
        assert!(!version_less_than("", "1.0.0"));
        assert!(!version_less_than("1.0.0", ""));
    }

    #[test]
    fn version_less_than_falls_back_to_lexicographic_for_non_numeric() {
        assert!(version_less_than("1.2a", "1.2b"));
        assert!(!version_less_than("1.2b", "1.2a"));
    }

    #[test]
    fn parse_catalog_reads_tsv_rows_and_skips_invalid() {
        let content = "battery.sh\tBattery Info\t1.2.1\tGitanElyon\tShows battery status\nweather.sh\tWeather\t1.0.0\tGitanElyon\tForecast\n\nbad-line\n";
        let entries = parse_catalog(content);

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].file, "battery.sh");
        assert_eq!(entries[0].version, "1.2.1");
        assert_eq!(entries[1].name, "Weather");
    }

    #[test]
    fn find_catalog_entry_matches_file_stem_and_name() {
        let entries = parse_catalog(
            "battery.sh\tBattery Info\t1.2.1\tA\tB\nweather.sh\tWeather\t1.0.0\tA\tB\n",
        );

        let by_file = find_catalog_entry(&entries, "battery.sh").unwrap();
        assert_eq!(by_file.file, "battery.sh");

        let by_stem = find_catalog_entry(&entries, "BATTERY").unwrap();
        assert_eq!(by_stem.file, "battery.sh");

        let by_name = find_catalog_entry(&entries, "Weather").unwrap();
        assert_eq!(by_name.file, "weather.sh");
    }

    #[test]
    fn find_catalog_entry_errors_on_missing_and_ambiguous() {
        let entries = parse_catalog("a.sh\tAlpha\t1.0.0\tA\tB\na.py\tPython\t1.0.0\tA\tB\n");

        assert!(find_catalog_entry(&entries, "nothing").is_err());

        let err = find_catalog_entry(&entries, "a").unwrap_err();
        assert!(err.contains("multiple"));
        assert!(err.contains("a.sh"));
        assert!(err.contains("a.py"));
    }

    #[test]
    fn safe_script_path_rejects_traversal_and_absolute_paths() {
        assert!(safe_script_path("battery.sh").is_ok());
        assert!(safe_script_path("sub/foo.sh").is_ok());
        assert!(safe_script_path("/tmp/x.sh").is_err());
        assert!(safe_script_path("../x.sh").is_err());
        assert!(safe_script_path("sub/../../x.sh").is_err());
    }
}
