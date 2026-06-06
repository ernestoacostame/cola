use crate::cli::LogFormat as CliLogFormat;
use crate::formats::apache::ApacheParser;
use crate::formats::nginx::NginxParser;
use crate::formats::syslog::SyslogParser;
use crate::formats::{LogParser, ParsedLine};
use regex::Regex;

pub struct ParserManager {
    parsers: Vec<Box<dyn LogParser>>,
    selected_format: CliLogFormat,
    detected_parser_index: Option<usize>,
    include_regex: Option<Regex>,
    exclude_regex: Option<Regex>,
}

impl ParserManager {
    pub fn new(
        format: CliLogFormat,
        include_pattern: Option<&str>,
        exclude_pattern: Option<&str>,
    ) -> anyhow::Result<Self> {
        let parsers: Vec<Box<dyn LogParser>> = vec![
            Box::new(NginxParser::new()),
            Box::new(ApacheParser::new()),
            Box::new(SyslogParser::new()),
        ];

        let include_regex = include_pattern
            .map(|pat| Regex::new(pat).map_err(anyhow::Error::from))
            .transpose()?;

        let exclude_regex = exclude_pattern
            .map(|pat| Regex::new(pat).map_err(anyhow::Error::from))
            .transpose()?;

        // If a specific format was chosen, detect the index immediately
        let detected_parser_index = match format {
            CliLogFormat::Auto => None,
            CliLogFormat::Nginx => Some(0),
            CliLogFormat::Apache => Some(1),
            CliLogFormat::Syslog => Some(2),
        };

        Ok(Self {
            parsers,
            selected_format: format,
            detected_parser_index,
            include_regex,
            exclude_regex,
        })
    }

    /// Filter a line using include/exclude regexes.
    /// Returns true if the line passes filters, false if it should be skipped.
    pub fn filter_line(&self, line: &str) -> bool {
        // Exclude filter: if it matches, we discard the line
        if let Some(ref re) = self.exclude_regex {
            if re.is_match(line) {
                return false;
            }
        }

        // Include filter: if it doesn't match, we discard the line
        if let Some(ref re) = self.include_regex {
            if !re.is_match(line) {
                return false;
            }
        }

        true
    }

    /// Parses a single line.
    /// Handles sticky auto-detection of format.
    /// Returns `Some((ParsedLine, parser_name))` or `None` if it could not be parsed.
    pub fn parse_line(&mut self, line: &str) -> Option<(ParsedLine, &'static str)> {
        // 1. If we have locked onto a parser, try that first
        if let Some(idx) = self.detected_parser_index {
            let parser = &self.parsers[idx];
            if let Some(parsed) = parser.parse(line) {
                return Some((parsed, parser.name()));
            }
            // If it failed but we were forced into a specific format, we don't try others
            if self.selected_format != CliLogFormat::Auto {
                return None;
            }
        }

        // 2. Otherwise (Auto-detecting and no lock yet, or lock failed under Auto mode)
        // Try each parser in order
        for (idx, parser) in self.parsers.iter().enumerate() {
            if let Some(parsed) = parser.parse(line) {
                // Lock onto this parser for future speed!
                if self.selected_format == CliLogFormat::Auto && self.detected_parser_index.is_none() {
                    self.detected_parser_index = Some(idx);
                }
                return Some((parsed, parser.name()));
            }
        }

        None
    }

    /// Returns the currently active parser's name, if any
    #[allow(dead_code)]
    pub fn active_parser_name(&self) -> Option<&'static str> {
        self.detected_parser_index.map(|idx| self.parsers[idx].name())
    }
}
