extern crate clap;
extern crate git2;

use clap::{App, Arg, AppSettings};
use std::env;
use std::path::{PathBuf};
use std::fs;
use std::os::linux::fs::MetadataExt;
use std::process::{Command};
use std::process::exit;
use std::io;
use git2::{Repository, Branch, StatusOptions, Status};

static COLOR_RED: &str = "\\[\\033[0;31m\\]";
static COLOR_CYAN: &str = "\\[\\033[0;1;36m\\]";
static COLOR_DARK_CYAN: &str = "\\[\\033[0;36m\\]";
static COLOR_YELLOW: &str = "\\[\\033[0;33m\\]";
static COLOR_GREEN: &str = "\\[\\033[0;32m\\]";
static COLOR_LIGHT_GREEN: &str = "\\[\\033[1;32m\\]";
static COLOR_LIGHT_BLUE: &str = "\\[\\033[1;34m\\]";
static COLOR_NONE: &str = "\\[\\033[0m\\]";
static COLOR_HOSTNAME: &str = "\\[\\033[38;5;28m\\]";

static FALLBACK_PROMPT: &str = "\\[\\033[0m\\]\\u@\\h \\w\\n$ ";

fn exit_with_fallback<E>(err: E) -> !
    where E: std::error::Error  {

    eprintln!("{}", err);
    println!("{}", FALLBACK_PROMPT);
    exit(1)
}

fn main() {
    let app = App::new("mkprompt")
        .arg(Arg::with_name("PATH")
             .help("use path instead of current working dir")
             .required(false)
             .index(1))
        .setting(AppSettings::DisableVersion);
    let matches = app.get_matches();

    let path = match matches.value_of("PATH") {
        Some(p) => PathBuf::from(p),
        None => env::current_dir().unwrap_or_else(|e| exit_with_fallback(e))
    }.canonicalize().unwrap_or_else(|e| exit_with_fallback(e));

    let path_str = path.to_str().unwrap_or_else(
        || exit_with_fallback(io::Error::new(
            io::ErrorKind::Other,
            "unable to convert path to string"
        ))
    );

    let root_filesystem_dev =
        fs::metadata("/").unwrap_or_else(|e| exit_with_fallback(e)).st_dev();
    let path_filesystem_dev =
        fs::metadata(path.clone()).unwrap_or_else(|e| exit_with_fallback(e)).st_dev();
    let on_root_filesystem = root_filesystem_dev == path_filesystem_dev;

    let git_prompt =
        if on_root_filesystem {
            get_git_prompt(&path).unwrap_or_else(|err| {
                eprintln!("{}", err);
                String::new()
            })
        } else {
            "".into()
        };

    println!("{}", [
        COLOR_LIGHT_GREEN.into(),
        "\\u".into(),
        if is_sudo_available() {
            format!("{}!", COLOR_RED)
        } else {
            format!("{}@", COLOR_HOSTNAME)
        },
        COLOR_HOSTNAME.into(),
        "\\h ".into(),
        COLOR_LIGHT_BLUE.into(),
        path_str.into(),
        COLOR_NONE.into(),
        " ".into(),
        git_prompt,
        "\\n".into(),
        "$ ".into(),
        COLOR_NONE.into()
    ].concat());
}

fn get_git_prompt(path: &PathBuf) -> Result<String, git2::Error> {
    if let Ok(mut git_repo) = Repository::discover(path) {

        let mut stash_count = 0;
        git_repo.stash_foreach(|_idx, _name, _oid| {
            stash_count += 1;
            true
        })?;

        let head = git_repo.head()?;
        let branch_opt = if head.is_branch() {
            Some(Branch::wrap(head))
        } else {
            None
        };
        let branch_name_opt: Option<String> =
            if let Some(branch) = &branch_opt {
                branch.name()?.map(|s| s.to_owned())
            } else {
                None
            };

        let statuses = git_repo.statuses(
            Some(StatusOptions::new()
                 .include_untracked(true)
                 .include_unmodified(false)
                 .renames_head_to_index(false)
                 .renames_index_to_workdir(false))
        )?;

        let aggregate_status = statuses.iter().fold(Status::empty(), |acc, se| acc | se.status());
        let has_staged_changes = aggregate_status.intersects(
            Status::INDEX_NEW |
            Status::INDEX_MODIFIED |
            Status::INDEX_DELETED |
            Status::INDEX_RENAMED |
            Status::INDEX_TYPECHANGE
        );
        let has_unstaged_changes = aggregate_status.intersects(
            Status::WT_NEW |
            Status::WT_MODIFIED |
            Status::WT_DELETED |
            Status::WT_RENAMED |
            Status::WT_TYPECHANGE
        );

        let (ahead, behind) =
            if let Some(branch) = &branch_opt {
                if let Ok(remote_branch) = branch.upstream() {
                    if let Some(branch_oid) = branch.get().target() {
                        if let Some(remote_branch_oid) = remote_branch.get().target() {
                            git_repo.graph_ahead_behind(branch_oid, remote_branch_oid)?
                        } else {
                            (0, 0)
                        }
                    } else {
                        (0, 0)
                    }
                } else {
                    (0, 0)
                }
            } else {
                (0, 0)
            };

        Ok([
            if branch_opt.is_none() {
                COLOR_RED.into()
            } else if has_unstaged_changes {
                if has_staged_changes {
                    COLOR_YELLOW.into()
                } else {
                    COLOR_CYAN.into()
                }
            } else {
                if has_staged_changes {
                    COLOR_DARK_CYAN.into()
                } else {
                    COLOR_GREEN.into()
                }
            },
            String::from("("),
            branch_name_opt.unwrap_or("detached".into()),
            String::from(")"),
            if ahead > 0 && behind == 0 {
                format!("{} ↑↑", COLOR_GREEN)
            } else if ahead == 0 && behind > 0 {
                format!("{} ↓↓↓↓", COLOR_RED)
            } else {
                "".into()
            },
            COLOR_RED.into(),
            if stash_count > 0 {
                stash_count.to_string()
            } else {
                "".into()
            },
            COLOR_NONE.into(),
        ].concat())
    } else {
        Ok("".into())
    }
}

fn is_sudo_available() -> bool {
    match env::var("HOME") {
        Err(err) => {
            eprintln!("{}", err);
            false
        },
        Ok(home) => {
            match Command::new(format!("{}/bin/checksudo", home)).status() {
                Err(err) => {
                    eprintln!("{}", err);
                    false
                },
                Ok(status) => {
                    status.success()
                }
            }
        }
    }
}
