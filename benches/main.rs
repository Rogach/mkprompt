#[macro_use] extern crate criterion;

use criterion::{Criterion};
use std::process::{Command, Stdio};

fn criterion_benchmark(c: &mut Criterion) {
    assert!(Command::new("/usr/bin/cargo").args(&["build", "--release"]).status().unwrap().success());
    c.bench_function(
        "run on release build",
        |b| b.iter(|| {
            assert!(
                Command::new("target/release/mkprompt")
                    .args(&["../bitcluster.ru"])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status().unwrap().success()
            );
        })
    );
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
