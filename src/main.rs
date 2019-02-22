extern crate clap;
extern crate git2;
#[macro_use] extern crate failure;

use clap::{App, Arg, AppSettings};
use std::env;
use std::path::{PathBuf, Component};
use std::fs;
use std::os::linux::fs::MetadataExt;
use std::process::{Command};
use std::process::exit;
use failure::Error;
use git2::{Repository, Branch, StatusOptions, Status};

static COLOR_RED: &str = "\\[\\033[0;31m\\]";
static COLOR_LIGHT_RED: &str = "\\[\\033[1;31m\\]";
static COLOR_GREEN: &str = "\\[\\033[0;32m\\]";
static COLOR_LIGHT_GREEN: &str = "\\[\\033[1;32m\\]";
static COLOR_YELLOW: &str = "\\[\\033[0;33m\\]";
static COLOR_BLUE: &str = "\\[\\033[0;34m\\]";
static COLOR_LIGHT_BLUE: &str = "\\[\\033[1;34m\\]";
static COLOR_CYAN: &str = "\\[\\033[0;1;36m\\]";
static COLOR_DARK_CYAN: &str = "\\[\\033[0;36m\\]";
static COLOR_NONE: &str = "\\[\\033[0m\\]";
static COLOR_HOSTNAME: &str = "\\[\\033[38;5;28m\\]";

static FALLBACK_PROMPT: &str = "\\[\\033[0m\\]\\u@\\h \\w";
static PWD_LENGTH_LIMIT: usize = 40;

fn exit_with_fallback(err: Error) -> ! {
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
        None => env::current_dir().unwrap_or_else(|e| exit_with_fallback(e.into()))
    }.canonicalize().unwrap_or_else(|e| exit_with_fallback(e.into()));

    let path_str = mkpwd(&path).unwrap_or_else(|e| exit_with_fallback(e.into()));

    let root_filesystem_dev =
        fs::metadata("/").unwrap_or_else(|e| exit_with_fallback(e.into())).st_dev();
    let path_filesystem_dev =
        fs::metadata(path.clone()).unwrap_or_else(|e| exit_with_fallback(e.into())).st_dev();
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
            format!("{}!", COLOR_LIGHT_RED)
        } else {
            format!("{}@", COLOR_HOSTNAME)
        },
        COLOR_HOSTNAME.into(),
        "\\h ".into(),
        path_str.into(),
        COLOR_NONE.into(),
        " ".into(),
        git_prompt
    ].concat());
}

fn get_git_prompt(path: &PathBuf) -> Result<String, git2::Error> {
    if let Ok(mut git_repo) = Repository::discover(path) {
        if git_repo.is_empty()? {
            return Ok([COLOR_RED, "(empty)", COLOR_NONE].concat());
        }
        if git_repo.is_bare() {
            return Ok([COLOR_RED, "(bare repository)", COLOR_NONE].concat());
        }

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
                format!(" ({})", stash_count)
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

fn mkpwd(path: &PathBuf) -> Result<String, Error> {
    let home = PathBuf::from(env::var("HOME")?);

    let mut remaining_length = path_length(path)?;
    let mut remaining_limit = PWD_LENGTH_LIMIT;
    let mut pwd = String::new();
    let mut under_limit = false;

    pwd.push_str(COLOR_BLUE);

    let path_components: Vec<Component> =
        if path.starts_with(&home) {
            remaining_limit -= 1;
            remaining_length -= path_length(&home)?;

            if !under_limit && remaining_length <= remaining_limit {
                pwd.push_str(COLOR_LIGHT_BLUE);
                under_limit = true;
            }
            pwd.push_str("~");

            path.components().skip((&home).components().count()).collect()
        } else {
            path.components().skip(0).collect()
        };


    for (i, c) in path_components.iter().enumerate() {
        match c {
            Component::RootDir => {
                if remaining_length <= remaining_limit {
                    if !under_limit {
                        pwd.push_str(COLOR_LIGHT_BLUE);
                        under_limit = true;
                    }
                }
                remaining_limit -= 1;
                remaining_length -= 1;
                pwd.push_str("/");
            },
            Component::Normal(s) => {
                let cs = s.to_str().ok_or(format_err!("unable to convert path to string"))?;
                let is_last_component = i == path_components.len() - 1;
                if !is_last_component && remaining_length > remaining_limit {
                    if !pwd.ends_with("/") {
                        remaining_limit -= 1;
                        remaining_length -= 1;
                        pwd.push_str("/");
                    }
                    remaining_limit -= 1;
                    remaining_length -= cs.len();
                    pwd.push_str(&cs.chars().take(1).collect::<String>());
                } else {
                    if !under_limit {
                        pwd.push_str(COLOR_LIGHT_BLUE);
                        under_limit = true;
                    }
                    if !pwd.ends_with("/") {
                        remaining_limit -= 1;
                        remaining_length -= 1;
                        pwd.push_str("/");
                    }
                    remaining_limit -= cs.len();
                    remaining_length -= cs.len();
                    pwd.push_str(cs);
                }
            },
            pp => return Err(format_err!("unexpected path part: {:?}", pp))
        };
    }

    Ok(pwd)
}

fn path_length(path: &PathBuf) -> Result<usize, Error> {
    let mut length = 0;
    for c in path.components() {
        match c {
            Component::RootDir => {},
            Component::Normal(s) => {
                length +=
                    s.to_str().ok_or(format_err!("unable to convert path to string"))?.len() + 1;
            },
            pp => return Err(format_err!("unexpected path part: {:?}", pp))
        }
    }
    if path.is_absolute() && length == 0 {
        length += 1;
    }
    Ok(length)
}
