use super::file_reader::FileReader;
use super::file_writer::FileWriter;
use super::general_error::Error;
use std::fmt;

use std::sync::Arc;
use std::sync::Mutex;

const LOG_MAX_SIZE : usize = 1024;

pub struct LogHandler<T : 'static + Log + Send> {
    log_file : String,
    stats    : Arc<Mutex<T>>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum ErrorKind {
    ParseError,
}

#[derive(Clone)]
pub struct LogError {
    kind : ErrorKind,
    path : String,
}

impl fmt::Display for LogError {
    fn fmt(&self, f : &mut fmt::Formatter) -> fmt::Result {
        use self::ErrorKind::*;
        match self.kind {
            ParseError => writeln!(f, "failed to parse log file \"{}\"", self.path),
        }
    }
}

impl LogError {
    pub fn new(kind : ErrorKind, path : &str) -> LogError {
        LogError {
            kind,
            path : String::from(path),
        }
    }
}

pub trait Log {
    fn serialize(&self) -> String;

    fn deserialize(&mut self, &[u8]) -> Result<(), ()>;

    fn read_from(&mut self, log_file : &str) -> Result<(), Error> {
        let mut reader = FileReader::new(log_file)?;
        let mut buffer : [u8; LOG_MAX_SIZE] = [0; LOG_MAX_SIZE];
        let _len_read = reader.read(&mut buffer)?;

        match self.deserialize(&buffer) {
            Ok(())  => Ok(()),
            Err(()) => Err(Error::with_message("failed to parse log")),
        }
    }
}