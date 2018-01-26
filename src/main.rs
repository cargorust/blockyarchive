#![allow(dead_code)]

#[macro_use]
extern crate nom;

extern crate time;

extern crate pond;

extern crate smallvec;
use smallvec::SmallVec;

#[macro_use]
extern crate reed_solomon_erasure;
use reed_solomon_erasure::ReedSolomon;

#[macro_use]
mod worker_macros;

mod file_error;
use file_error::FileError;

mod general_error;
use general_error::Error;
use general_error::ErrorKind;

mod multihash;
mod multihash_test;
mod misc_utils;
mod misc_utils_test;
mod rand_utils;
mod sbx_block;
mod sbx_specs;

mod encode_core;
mod decode_core;
mod rescue_core;
mod repair_core;
mod show_core;
mod sort_core;

mod progress_report;

mod file_reader;
use file_reader::FileReader;
mod file_writer;
use file_writer::FileWriter;

mod worker;

fn main () {
    use encode_core::Param;
    let param = Param {
        version : sbx_specs::Version::V1,
        file_uid : [0, 1, 2, 3, 4, 5],
        rs_enabled : true,
        rs_data    : 10,
        rs_parity  : 2,
        hash_enabled : true,
        hash_type  : multihash::HashType::SHA256,
        in_file    : String::from("test"),
        out_file   : String::from("test.sbx")
    };
    match encode_core::encode_file(&param) {
        Ok(_)  => {},
        Err(e) => println!("Error : {}", e)
    }
}
