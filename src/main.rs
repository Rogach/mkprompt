extern crate clap;

use clap::{App, Arg, AppSettings};
use std::env;
use std::path::{PathBuf};

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

    println!("{:?}", path);
}
