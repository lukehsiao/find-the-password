use std::collections::HashMap;
use std::io::{self, Read};
use std::str::FromStr;

use anyhow::{anyhow, Error, Result};

#[derive(Debug)]
struct Verse {
    book: String,
    chapter: u32,
    verse: u32,
    text: String,
}

impl FromStr for Verse {
    type Err = Error;

    /// Take an example plain-text verse as input and store it as a Verse.
    ///
    /// Input verses follow the format from Project Gutenberg's plain text formats. For example:
    /// ```
    /// Moroni 10:32
    ///  32 Yea, come unto Christ, and be perfected in him, and deny
    /// yourselves of all ungodliness; and if ye shall deny yourselves of
    /// all ungodliness and love God with all your might, mind and
    /// strength, then is his grace sufficient for you, that by his grace
    /// ye may be perfect in Christ; and if by the grace of God ye are
    /// perfect in Christ, ye can in nowise deny the power of God.
    /// ```
    fn from_str(s: &str) -> Result<Verse> {
        let mut lines = s.trim().lines();

        // Parse book, chapter, verse
        let (book, chapter, verse) = lines
            .next()
            .map(|meta| {
                let v: Vec<&str> = meta.split(' ').collect();
                let book: String = v[0].to_string();
                let v: Vec<u32> = v[1].split(':').map(|n| n.parse::<u32>().unwrap()).collect();
                (book, v[0], v[1])
            })
            .unwrap();

        // Parse the verse
        let (_, text) = lines.fold((0, String::new()), |mut acc, l| {
            if acc.0 == 0 {
                // Skip the verse number on the first line
                let word_vec: Vec<&str> = l.trim().split(' ').collect();
                let line = &word_vec[1..].join(" ");
                acc.1.push_str(line);
            } else {
                acc.1.push(' ');
                acc.1.push_str(l);
            }
            acc.0 += 1;
            (acc.0, acc.1)
        });

        Ok(Verse {
            book,
            chapter,
            verse,
            text,
        })
    }
}

fn main() {
    println!("Hello, world!");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_str() -> Result<()> {
        let input = "
Moroni 10:32
 32 Yea, come unto Christ, and be perfected in him, and deny
yourselves of all ungodliness; and if ye shall deny yourselves of
all ungodliness and love God with all your might, mind and
strength, then is his grace sufficient for you, that by his grace
ye may be perfect in Christ; and if by the grace of God ye are
perfect in Christ, ye can in nowise deny the power of God.
        ";

        let verse: Verse = input.parse()?;
        assert_eq!("Moroni", &verse.book);
        assert_eq!(10, verse.chapter);
        assert_eq!(32, verse.verse);
        assert_eq!("Yea, come unto Christ, and be perfected in him, and deny yourselves of all ungodliness; and if ye shall deny yourselves of all ungodliness and love God with all your might, mind and strength, then is his grace sufficient for you, that by his grace ye may be perfect in Christ; and if by the grace of God ye are perfect in Christ, ye can in nowise deny the power of God.", &verse.text);

        Ok(())
    }
}
