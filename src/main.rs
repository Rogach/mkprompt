extern crate clap;
extern crate git2;

use clap::{App, Arg, AppSettings};
use std::env;
use std::path::{PathBuf};
use std::fs;
use std::os::linux::fs::MetadataExt;
use git2::{Repository, Branch};

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
        None => env::current_dir().unwrap()
    }.canonicalize().unwrap();

    let root_filesystem_dev = fs::metadata("/").unwrap().st_dev();
    let path_filesystem_dev = fs::metadata(path.clone()).unwrap().st_dev();
    let on_root_filesystem = root_filesystem_dev == path_filesystem_dev;

    if on_root_filesystem {
        if let Ok(git_repo) = Repository::discover(path.clone()) {
            let head = git_repo.head().unwrap();
            let branch_name_opt: Option<String> = if head.is_branch() {
                Branch::wrap(head).name().unwrap().map(|s| s.to_owned())
            } else {
                None
            };
            println!("{:?}", branch_name_opt);
        }
    }
}
