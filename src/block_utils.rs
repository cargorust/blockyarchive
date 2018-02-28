use super::sbx_specs::SBX_LARGEST_BLOCK_SIZE;
use super::sbx_specs::SBX_SCAN_BLOCK_SIZE;
use super::sbx_block::Block;
use super::file_reader::FileReader;
use super::file_reader::FileReaderParam;
use super::file_writer::FileWriter;
use super::file_writer::FileWriterParam;
use super::sbx_block::BlockType;

use std::sync::{Arc, Mutex};
use std::fs;
use super::file_utils;

use super::progress_report::*;

use super::sbx_specs::ver_to_block_size;
use super::Error;

pub struct ReadResult {
    pub len_read : usize,
    pub usable   : bool,
    pub eof      : bool,
}

struct ScanStats {
    pub bytes_processed : u64,
    pub total_bytes     : u64,
    start_time          : f64,
    end_time            : f64,
}

impl ScanStats {
    pub fn new(file_metadata : &fs::Metadata) -> ScanStats {
        ScanStats {
            bytes_processed : 0,
            total_bytes     : file_metadata.len(),
            start_time      : 0.,
            end_time        : 0.,
        }
    }
}

impl ProgressReport for ScanStats {
    fn start_time_mut(&mut self) -> &mut f64 { &mut self.start_time }

    fn end_time_mut(&mut self)   -> &mut f64 { &mut self.end_time }

    fn units_so_far(&self)       -> u64      { self.bytes_processed }

    fn total_units(&self)        -> u64      { self.total_bytes }
}

pub fn read_block_lazily(block  : &mut Block,
                         buffer : &mut [u8; SBX_LARGEST_BLOCK_SIZE],
                         reader : &mut FileReader)
                         -> Result<ReadResult, Error> {
    let mut total_len_read = 0;

    { // scan at 128 chunk size
        total_len_read += reader.read(&mut buffer[0..SBX_SCAN_BLOCK_SIZE])?;

        if total_len_read < SBX_SCAN_BLOCK_SIZE {
            return Ok(ReadResult { len_read : total_len_read,
                                   usable   : false,
                                   eof      : true            });
        }

        match block.sync_from_buffer_header_only(&buffer[0..SBX_SCAN_BLOCK_SIZE]) {
            Ok(()) => {},
            Err(_) => { return Ok(ReadResult { len_read : total_len_read,
                                               usable   : false,
                                               eof      : false           }); }
        }
    }

    { // get remaining bytes of block if necessary
        let block_size = ver_to_block_size(block.get_version());

        total_len_read +=
            reader.read(&mut buffer[SBX_SCAN_BLOCK_SIZE..block_size])?;

        if total_len_read < block_size {
            return Ok(ReadResult { len_read : total_len_read,
                                   usable   : false,
                                   eof      : true            });
        }

        match block.sync_from_buffer(&buffer[0..block_size]) {
            Ok(()) => {},
            Err(_) => { return Ok(ReadResult { len_read : total_len_read,
                                               usable   : false,
                                               eof      : false           }); }
        }
    }

    Ok(ReadResult { len_read : total_len_read,
                    usable   : true,
                    eof      : false           })
}

pub fn get_ref_block(in_file            : &str,
                     use_any_block_type : bool,
                     silence_level      : SilenceLevel)
                     -> Result<Option<(u64, Block)>, Error> {
    let metadata = file_utils::get_file_metadata(in_file)?;

    let stats = Arc::new(Mutex::new(ScanStats::new(&metadata)));

    let reporter = ProgressReporter::new(&stats,
                                         "Reference block scanning progress",
                                         "bytes",
                                         silence_level);

    let mut buffer : [u8; SBX_LARGEST_BLOCK_SIZE] =
        [0; SBX_LARGEST_BLOCK_SIZE];

    let mut block = Block::dummy();

    let mut meta_block = None;
    let mut data_block = None;

    let mut reader = FileReader::new(in_file,
                                     FileReaderParam { write    : false,
                                                       buffered : true   })?;

    reporter.start();

    loop {
        let lazy_read_res = read_block_lazily(&mut block,
                                              &mut buffer,
                                              &mut reader)?;

        stats.lock().unwrap().bytes_processed += lazy_read_res.len_read as u64;

        if lazy_read_res.eof     { break; }

        if !lazy_read_res.usable { continue; }

        match block.block_type() {
            BlockType::Meta => {
                if let None = meta_block {
                    meta_block = Some(block.clone());
                }
            },
            BlockType::Data => {
                if let None = data_block {
                    data_block = Some(block.clone());
                }
            }
        }

        if use_any_block_type {
            if let Some(_) = meta_block { break; }
            if let Some(_) = data_block { break; }
        } else {
            if let Some(_) = meta_block { break; }
        }
    }

    reporter.stop();

    Ok(if     let Some(x) = meta_block {
        Some((stats.lock().unwrap().bytes_processed, x))
    } else if let Some(x) = data_block {
        Some((stats.lock().unwrap().bytes_processed, x))
    } else {
        None
    })
}