use std::{
    io::{self, Read},
    process,
};
use tracing::info;

use anyhow::{anyhow, Result};
use rayon::prelude::*;

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    let urls: Vec<String> = input
        .lines()
        .map(|n| n.trim().to_string())
        .map(|pass| format!("https://challenge.hsiao.dev/03/u/luke/check/{pass}"))
        .collect();

    urls.par_iter().for_each(|n| {
        info!(url = %n, "Trying URL");
        if let Ok(res) = ureq::get(&n).call() {
            if res.into_string().unwrap() == "True" {
                let pass = n.rsplit("/").next().unwrap();
                println!("Password is: {pass}");
                // Terminate immediately
                process::exit(0);
            }
        }
    });

    Err(anyhow!("Didn't find the password :("))
}
