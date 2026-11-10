//! The support report (roadmap 11.4): a plain text file the clinic can send to Artaveo support.
//! It holds the version, the system, the state of the database and backups, and the tail of the
//! newest log file. Patient data is never part of it: the log never carries any (see `logging`), and
//! the report has no database content at all. The user decides whether to send it; the program
//! sends nothing by itself.

use std::fs;
use std::path::PathBuf;

use crate::audit;
use crate::clock::{now, now_iso};
use crate::config::Config;
use crate::error::Result;
use crate::sysinfo;

/// How many of the newest log lines go into the report.
const LOG_LINES: usize = 300;
/// A log line is never longer than this in the report.
const LOG_LINE_CHARS: usize = 600;

/// The written report.
#[derive(Debug, Clone)]
pub struct ReportFile {
    pub file_name: String,
    pub path: PathBuf,
    pub bytes: u64,
}

/// Facts that need no database, so the report also works when the clinic does not open.
pub fn base_facts(config: &Config) -> Vec<(String, String)> {
    let build = sysinfo::build_arch();
    let machine = sysinfo::machine_arch();
    vec![
        ("app_version".into(), crate::APP_VERSION.into()),
        ("git_commit".into(), crate::GIT_COMMIT.into()),
        ("build_arch".into(), build.into()),
        ("machine_arch".into(), machine),
        ("os".into(), sysinfo::os_description()),
        ("environment".into(), config.environment.code().into()),
        ("computer_name".into(), audit::computer_name().into()),
        ("data_dir".into(), config.data_dir.display().to_string()),
    ]
}

fn newest_log(config: &Config) -> Option<PathBuf> {
    logs_newest_last(config).pop()
}

/// The newest log files, newest last (the date is in the name).
fn logs_newest_last(config: &Config) -> Vec<PathBuf> {
    let mut logs: Vec<PathBuf> = fs::read_dir(config.log_dir())
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("artaveo.") && n.ends_with(".log"))
        })
        .collect();
    logs.sort();
    logs
}

/// How many error lines of the last two log files go into the report.
const ERROR_LINES: usize = 80;

/// Only the error lines (the Core's request failures with their reference, UI errors, panics), from the
/// last two days: the first place support looks.
fn recent_errors(config: &Config) -> String {
    let logs = logs_newest_last(config);
    let from = logs.len().saturating_sub(2);
    let mut lines: Vec<String> = Vec::new();
    for path in &logs[from..] {
        let Ok(bytes) = fs::read(path) else { continue };
        let text = String::from_utf8_lossy(&bytes);
        lines.extend(
            text.lines()
                .filter(|l| l.contains("\"level\":\"ERROR\""))
                .map(|l| l.chars().take(LOG_LINE_CHARS).collect::<String>()),
        );
    }
    let skip = lines.len().saturating_sub(ERROR_LINES);
    if lines.is_empty() {
        "(no errors in the last two log files)".into()
    } else {
        lines[skip..].join("\n")
    }
}

/// Crash files (crashes/crash-*.txt, written by the panic hook): name and the where line.
fn crash_summary(config: &Config) -> String {
    let dir = config.data_dir.join("crashes");
    let mut files: Vec<PathBuf> =
        fs::read_dir(&dir).into_iter().flatten().filter_map(|e| e.ok()).map(|e| e.path()).collect();
    files.sort();
    if files.is_empty() {
        return "(no crash files)".into();
    }
    let from = files.len().saturating_sub(10);
    files[from..]
        .iter()
        .map(|f| {
            let name = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let place = fs::read_to_string(f)
                .ok()
                .and_then(|t| t.lines().find(|l| l.starts_with("where:")).map(str::to_string))
                .unwrap_or_default();
            format!("{name} {place}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn log_tail(config: &Config) -> String {
    let Some(path) = newest_log(config) else { return "(no log file)".into() };
    let Ok(bytes) = fs::read(&path) else { return "(the log could not be read)".into() };
    let text = String::from_utf8_lossy(&bytes);
    let lines: Vec<&str> = text.lines().collect();
    let from = lines.len().saturating_sub(LOG_LINES);
    lines[from..]
        .iter()
        .map(|l| l.chars().take(LOG_LINE_CHARS).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Writes the report into the exports folder (the only folder the window may show in Explorer).
pub fn write_report(config: &Config, facts: &[(String, String)]) -> Result<ReportFile> {
    let dir = config.exports_dir();
    fs::create_dir_all(&dir)?;
    let t = now();
    let file_name = format!(
        "artaveo-support-{:04}{:02}{:02}-{:02}{:02}{:02}.txt",
        t.year(),
        t.month() as u8,
        t.day(),
        t.hour(),
        t.minute(),
        t.second()
    );
    let mut text = String::new();
    text.push_str("Artaveo Dental support report\n");
    text.push_str(&format!("created_at: {}\n", now_iso()));
    text.push_str("This file has no patient data.\n\n");
    for (key, value) in facts {
        text.push_str(&format!("{key}: {value}\n"));
    }
    text.push_str("\n--- crash files (full text stays on the computer in the crashes folder) ---\n");
    text.push_str(&crash_summary(config));
    text.push_str("\n\n--- errors in the last two log files ---\n");
    text.push_str(&recent_errors(config));
    text.push_str("\n\n--- newest log lines ---\n");
    text.push_str(&log_tail(config));
    text.push('\n');
    let path = dir.join(&file_name);
    fs::write(&path, &text)?;
    Ok(ReportFile { file_name, path, bytes: text.len() as u64 })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Environment;

    #[test]
    fn the_report_has_the_facts_and_the_log_tail_and_goes_into_the_exports_folder() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::new(Environment::Test, dir.path().to_path_buf());
        fs::create_dir_all(config.log_dir()).unwrap();
        let lines: Vec<String> = (0..500).map(|i| format!("{{\"line\":{i}}}")).collect();
        fs::write(config.log_dir().join("artaveo.2026-11-09.log"), "old day\n").unwrap();
        fs::write(config.log_dir().join("artaveo.2026-11-10.log"), lines.join("\n")).unwrap();

        let mut facts = base_facts(&config);
        facts.push(("startup_problem".into(), "damaged".into()));
        let report = write_report(&config, &facts).unwrap();
        assert_eq!(report.path.parent().unwrap(), config.exports_dir());
        let text = fs::read_to_string(&report.path).unwrap();
        assert!(text.contains(&format!("app_version: {}", crate::APP_VERSION)));
        assert!(text.contains("startup_problem: damaged"));
        assert!(text.contains("{\"line\":499}") && text.contains("{\"line\":200}"));
        assert!(!text.contains("{\"line\":199}"), "only the newest 300 lines");
        assert!(!text.contains("old day"), "only the newest log file");
        assert_eq!(report.bytes as usize, text.len());
    }

    #[test]
    fn errors_and_crashes_get_their_own_sections() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::new(Environment::Test, dir.path().to_path_buf());
        fs::create_dir_all(config.log_dir()).unwrap();
        fs::create_dir_all(dir.path().join("crashes")).unwrap();
        let log = "{\"level\":\"INFO\",\"message\":\"fine\"}\n{\"level\":\"ERROR\",\"message\":\"request failed\",\"reference\":\"K7M2QX\"}\n";
        fs::write(config.log_dir().join("artaveo.2026-11-10.log"), log).unwrap();
        fs::write(
            dir.path().join("crashes/crash-1.txt"),
            "Artaveo Dental crash\nwhere: core/src/billing.rs:10:5\n",
        )
        .unwrap();
        let report = write_report(&config, &base_facts(&config)).unwrap();
        let text = fs::read_to_string(report.path).unwrap();
        let errors = text.split("--- errors in the last two log files ---").nth(1).unwrap();
        let errors = errors.split("--- newest log lines ---").next().unwrap();
        assert!(errors.contains("K7M2QX") && !errors.contains("fine"));
        assert!(text.contains("crash-1.txt where: core/src/billing.rs:10:5"));
    }

    #[test]
    fn a_missing_log_is_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::new(Environment::Test, dir.path().to_path_buf());
        let report = write_report(&config, &base_facts(&config)).unwrap();
        assert!(fs::read_to_string(report.path).unwrap().contains("(no log file)"));
    }
}
