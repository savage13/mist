use crate::timer::Run;
use ron::de::from_str;
use ron::ser::{to_writer_pretty, PrettyConfig};
use serde::Deserialize;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::str::FromStr;

#[derive(Deserialize)]
struct LegacyRun {
    game_title: String,
    category: String,
    offset: Option<u128>,
    pb: u128,
    splits: Vec<String>,
    pb_times: Vec<u128>,
    gold_times: Vec<u128>,
}

#[derive(Deserialize)]
struct RunV1 {
    game_title: String,
    category: String,
    offset: Option<u128>,
    pb: u128,
    splits: Vec<String>,
    pb_times: Vec<u128>,
    gold_times: Vec<u128>,
    sum_times: Vec<(u128, u128)>,
}

impl Into<Run> for LegacyRun {
    fn into(self) -> Run {
        Run::new(
            self.category,
            self.game_title,
            self.offset.into(),
            self.pb.into(),
            &self.splits,
            &self.pb_times.iter().map(|&t| t.into()).collect::<Vec<_>>(),
            &self
                .gold_times
                .iter()
                .map(|&t| t.into())
                .collect::<Vec<_>>(),
            &self
                .pb_times
                .iter()
                .map(|&t| (1u128, t.into()))
                .collect::<Vec<_>>(),
        )
    }
}

impl Into<Run> for RunV1 {
    fn into(self) -> Run {
        Run::new(
            self.category,
            self.game_title,
            self.offset.into(),
            self.pb.into(),
            &self.splits,
            &self.pb_times.iter().map(|&t| t.into()).collect::<Vec<_>>(),
            &self
                .gold_times
                .iter()
                .map(|&t| t.into())
                .collect::<Vec<_>>(),
            &self
                .sum_times
                .iter()
                .map(|&(n, t)| (n, t.into()))
                .collect::<Vec<_>>(),
        )
    }
}

/// Parses the version and [`Run`] from a mist split file (msf).
pub struct MsfParser {
    filename: String,
}

impl MsfParser {
    /// Create a new [`MsfParser`].
    pub fn new(filename: String) -> Self {
        Self { filename }
    }

    /// Attempt to parse a [`Run`] from the given reader. Reader must implement [`BufRead`].
    ///
    /// If the file does not specify version in the first line, it is assumed to be a legacy (i.e. not up to date) run
    /// and is treated as such. Runs converted from legacy runs will have the new field(s) filled but zeroed.
    ///
    /// # Errors
    ///
    /// * If the reader cannot be read from or is empty.
    /// * If a [`Run`] (legacy or otherwise) cannot be parsed from the reader.
    pub fn parse(&self) -> Result<Run, String> {
        let f = File::open(&self.filename).map_err(|e| e.to_string())?;
        let mut lines = BufReader::new(f).lines().map(|l| l.unwrap());
        // TODO: better error handling
        let ver_info = String::from_str(&lines.next().ok_or("Input was empty.")?).unwrap();
        let version: u32 = match ver_info.rsplit_once(' ') {
            Some(num) => num.1.parse::<u32>().unwrap_or(0),
            None => 0,
        };
        let data = {
            let mut s = String::new();
            if version == 0 {
                s.push_str(&ver_info);
            }
            for line in lines {
                s.push_str(&line);
                s.push('\n');
            }
            s
        };
        let run = match version {
            1 => from_str::<RunV1>(&data).map_err(|e| e.to_string())?.into(),
            2 => from_str::<Run>(&data).map_err(|e| e.to_string())?,
            _ => from_str::<LegacyRun>(&data)
                .map_err(|e| e.to_string())?
                .into(),
        };
        Ok(run)
    }

    /// Write the given run to the given writer.
    pub fn write<W: Write>(&mut self, run: &Run) -> Result<(), String> {
        let run = super::sanify_run(run);
        let mut file = File::create(&self.filename).map_err(|e| e.to_string())?;
        file.write(b"version 2\n").map_err(|e| e.to_string())?;
        to_writer_pretty(&mut file, &run, PrettyConfig::new()).map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const V1RUN: &[u8] = b"version 1\n
        (
            game_title: \"test\",
            category: \"test\",
            offset: Some(200),
            pb: 1234,
            splits: [\"test\"],
            pb_times: [1234],
            gold_times: [1234],
            sum_times: [(2, 2480)],
        )";
    #[test]
    fn test_parse() {
        let reader = std::io::BufReader::new(V1RUN);
        let parser = MsfParser::new();
        let run = parser.parse(reader);
        println!("{:?}", run);
        assert!(run.is_ok());
    }

    const LEGACYRUN: &[u8] = b"(
        game_title: \"test\",
        category: \"test\",
        offset: Some(200),
        pb: 1234,
        splits: [\"test\"],
        pb_times: [1234],
        gold_times: [1234],
    )";

    #[test]
    fn test_parse_legacy() {
        let reader = std::io::BufReader::new(LEGACYRUN);
        let parser = MsfParser::new();
        let run = parser.parse(reader);
        assert!(run.is_ok());
    }

    const INSANE_RUN: &[u8] = b"version 1\n
        (
            game_title: \"test\",
            category: \"test\",
            offset: Some(200),
            pb: 1234,
            splits: [\"test\", \"test2\"],
            pb_times: [1234],
            gold_times: [1234],
            sum_times: [(2, 1234)],
        )";

    #[test]
    fn test_sanity_check() {
        let reader = std::io::BufReader::new(INSANE_RUN);
        let parser = MsfParser::new();
        let run = parser.parse(reader);
        assert!(run.is_ok());
        let run = run.unwrap();
        assert_eq!(run.gold_times().to_owned(), vec![1234, 0]);
        assert_eq!(run.pb_times().to_owned(), vec![1234, 0]);
        assert_eq!(run.sum_times().to_owned(), vec![(2, 1234), (0, 0)]);
    }
}
