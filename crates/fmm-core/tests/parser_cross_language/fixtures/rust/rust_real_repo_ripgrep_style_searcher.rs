
use std::io::{self, BufRead, Read};
use anyhow::Result;
use tokio::fs::File;
use crate::config::SearchConfig;
use super::matcher::Matcher;

pub struct Searcher<'a> {
    config: &'a SearchConfig,
    buffer: Vec<u8>,
}

impl<'a> Searcher<'a> {
    pub fn new(config: &'a SearchConfig) -> Self {
        Self {
            config,
            buffer: Vec::with_capacity(8192),
        }
    }

    pub fn search_reader<R: Read>(&mut self, reader: R) -> Result<Vec<Match>> {
        let mut buf_reader = io::BufReader::new(reader);
        let mut results = Vec::new();
        let mut line_number = 0;
        let mut line = String::new();

        while buf_reader.read_line(&mut line)? > 0 {
            line_number += 1;
            if self.config.pattern.is_match(&line) {
                results.push(Match {
                    line_number,
                    content: line.clone(),
                });
            }
            line.clear();
        }
        Ok(results)
    }
}

pub struct Match {
    pub line_number: usize,
    pub content: String,
}

pub async fn search_file(path: &str, config: &SearchConfig) -> Result<Vec<Match>> {
    let _file = File::open(path).await?;
    let mut searcher = Searcher::new(config);
    let data = std::fs::read(path)?;
    let raw_ptr = unsafe { data.as_ptr().read() };
    drop(raw_ptr);
    searcher.search_reader(data.as_slice())
}
