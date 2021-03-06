use crate::block_utils::RefBlockChoice;
use crate::cli_utils::setup_ctrlc_handler;
use crate::file_reader::{FileReader, FileReaderParam};
use crate::file_utils;
use crate::general_error::Error;
use crate::json_printer::{BracketType, JSONPrinter};
use crate::progress_report::*;
use crate::reader::ReadResult;
use crate::rs_codec::RSCodecState;
use crate::rs_codec::RSRepairer;
use crate::sbx_block;
use crate::sbx_block::Block;
use crate::sbx_block::BlockType;
use crate::sbx_block::Header;
use crate::sbx_specs::Version;
use crate::sbx_specs::SBX_LARGEST_BLOCK_SIZE;
use crate::sbx_specs::{ver_to_block_size, ver_to_usize};
use crate::time_utils;
use std::fmt;
use std::io::SeekFrom;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub struct Stats {
    version: Version,
    block_size: u64,
    pub meta_blocks_decoded: u64,
    pub data_or_par_blocks_decoded: u64,
    pub blocks_decode_failed: u64,
    pub meta_blocks_repaired: u64,
    pub data_or_par_blocks_repaired: u64,
    pub data_or_par_blocks_repair_failed: u64,
    total_blocks: u64,
    start_time: f64,
    end_time: f64,
    json_printer: Arc<JSONPrinter>,
}

impl Stats {
    pub fn new(ref_block: &Block, total_blocks: u64, json_printer: &Arc<JSONPrinter>) -> Stats {
        let version = ref_block.get_version();
        Stats {
            version,
            block_size: ver_to_block_size(version) as u64,
            blocks_decode_failed: 0,
            meta_blocks_decoded: 0,
            data_or_par_blocks_decoded: 0,
            meta_blocks_repaired: 0,
            data_or_par_blocks_repaired: 0,
            data_or_par_blocks_repair_failed: 0,
            total_blocks,
            start_time: 0.,
            end_time: 0.,
            json_printer: Arc::clone(json_printer),
        }
    }

    fn blocks_so_far(&self) -> u64 {
        self.meta_blocks_decoded + self.data_or_par_blocks_decoded + self.blocks_decode_failed
    }
}

impl ProgressReport for Stats {
    fn start_time_mut(&mut self) -> &mut f64 {
        &mut self.start_time
    }

    fn end_time_mut(&mut self) -> &mut f64 {
        &mut self.end_time
    }

    fn units_so_far(&self) -> u64 {
        self.blocks_so_far() * self.block_size
    }

    fn total_units(&self) -> Option<u64> {
        Some(self.total_blocks * self.block_size)
    }
}

impl fmt::Display for Stats {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let block_size = ver_to_block_size(self.version);
        let time_elapsed = (self.end_time - self.start_time) as i64;
        let (hour, minute, second) = time_utils::seconds_to_hms(time_elapsed);

        let json_printer = &self.json_printer;

        json_printer.write_open_bracket(f, Some("stats"), BracketType::Curly)?;

        write_maybe_json!(
            f,
            json_printer,
            "SBX version                              : {}",
            ver_to_usize(self.version)
        )?;
        write_maybe_json!(
            f,
            json_printer,
            "Block size used in checking              : {}",
            block_size
        )?;
        write_maybe_json!(
            f,
            json_printer,
            "Number of blocks processed               : {}",
            self.blocks_so_far()
        )?;
        write_maybe_json!(
            f,
            json_printer,
            "Number of blocks passed check (metadata) : {}",
            self.meta_blocks_decoded
        )?;
        write_maybe_json!(
            f,
            json_printer,
            "Number of blocks passed check (data)     : {}",
            self.data_or_par_blocks_decoded
        )?;
        write_maybe_json!(
            f,
            json_printer,
            "Number of blocks failed check            : {}",
            self.blocks_decode_failed
        )?;
        write_maybe_json!(
            f,
            json_printer,
            "Number of blocks repaired (metadata)     : {}",
            self.meta_blocks_repaired
        )?;
        write_maybe_json!(
            f,
            json_printer,
            "Number of blocks repaired (data)         : {}",
            self.data_or_par_blocks_repaired
        )?;
        write_maybe_json!(
            f,
            json_printer,
            "Number of blocks failed to repair (data) : {}",
            self.data_or_par_blocks_repair_failed
        )?;
        write_maybe_json!(
            f,
            json_printer,
            "Time elapsed                             : {:02}:{:02}:{:02}",
            hour,
            minute,
            second
        )?;

        if self.blocks_decode_failed == 0 {
            write_if!(not_json => f, json_printer => "No repairs required";)?;
        } else {
            if self.data_or_par_blocks_repair_failed == 0 {
                write_if!(not_json => f, json_printer => "All corrupted/missing blocks were repaired successfully";)?;
            } else {
                if self.blocks_decode_failed == self.data_or_par_blocks_repair_failed {
                    write_if!(not_json => f, json_printer => "All repairs failed";)?;
                } else {
                    write_if!(not_json => f, json_printer => "Some repairs failed";)?;
                }
            }
        }

        json_printer.write_close_bracket(f)?;

        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct Param {
    in_file: String,
    dry_run: bool,
    json_printer: Arc<JSONPrinter>,
    verbose: bool,
    pr_verbosity_level: PRVerbosityLevel,
    burst: Option<usize>,
}

impl Param {
    pub fn new(
        in_file: &str,
        dry_run: bool,
        json_printer: &Arc<JSONPrinter>,
        verbose: bool,
        pr_verbosity_level: PRVerbosityLevel,
        burst: Option<usize>,
    ) -> Param {
        Param {
            in_file: String::from(in_file),
            dry_run,
            json_printer: Arc::clone(json_printer),
            verbose,
            pr_verbosity_level,
            burst,
        }
    }
}

fn update_rs_codec_and_stats(
    version: Version,
    header_pred: &dyn Fn(&Header) -> bool,
    read_res: &ReadResult,
    block: &mut Block,
    cur_seq_num: u32,
    rs_codec: &mut RSRepairer,
    stats: &mut Stats,
) -> RSCodecState {
    let block_size = ver_to_block_size(version);

    if read_res.len_read < block_size {
        // read an incomplete block
        stats.blocks_decode_failed += 1;
        rs_codec.mark_missing()
    } else if let Err(_) =
        block.sync_from_buffer(rs_codec.get_block_buffer(), Some(header_pred), None)
    {
        stats.blocks_decode_failed += 1;
        rs_codec.mark_missing()
    } else {
        if block.get_seq_num() != cur_seq_num {
            stats.blocks_decode_failed += 1;
            rs_codec.mark_missing()
        } else {
            if block.is_meta() {
                stats.meta_blocks_decoded += 1;
            } else {
                stats.data_or_par_blocks_decoded += 1;
            }

            rs_codec.mark_present()
        }
    }
}

fn repair_blocks_and_update_stats_using_repair_stats(
    param: &Param,
    cur_seq_num: u32,
    rs_codec: &mut RSRepairer,
    stats: &mut Stats,
    reader: &mut FileReader,
    reporter: &ProgressReporter<Stats>,
) -> Result<(), Error> {
    let (repair_stats, repaired_blocks) = rs_codec.repair_with_block_sync(cur_seq_num);

    if repair_stats.successful {
        stats.data_or_par_blocks_repaired += repair_stats.missing_count as u64;
    } else {
        stats.data_or_par_blocks_repair_failed += repair_stats.missing_count as u64;
    }

    if repair_stats.missing_count > 0 {
        print_if!(verbose => param, reporter =>
                  "{}", repair_stats;);
    }

    if !param.dry_run {
        // write the repaired data blocks
        for &(pos, block_buf) in repaired_blocks.iter() {
            reader.seek(SeekFrom::Start(pos))?;
            reader.write(&block_buf)?;
        }
    }

    Ok(())
}

pub fn repair_file(param: &Param) -> Result<Option<Stats>, Error> {
    let ctrlc_stop_flag = setup_ctrlc_handler(param.json_printer.json_enabled());

    let json_printer = &param.json_printer;

    let (ref_block_pos, mut ref_block) = get_ref_block!( no_force_misalign =>
        param,
        None,
        None,
        json_printer,
        RefBlockChoice::MustBe(BlockType::Meta),
        ctrlc_stop_flag
    );

    let version = ref_block.get_version();

    return_if_not_ver_uses_rs!(version, json_printer);

    let block_size = ver_to_block_size(version);

    let data_par_burst =
        get_data_par_burst!(no_offset => param, ref_block_pos, ref_block, "repair");

    let total_block_count = {
        use crate::file_utils::from_orig_file_size::calc_total_block_count_exc_burst_gaps;
        match ref_block.get_FSZ().unwrap() {
            Some(x) => calc_total_block_count_exc_burst_gaps(version, None, data_par_burst, x),
            None => {
                print_if!(not_json => json_printer =>
                          "";
                          "Warning :";
                          "";
                          "    No recorded file size found, using container file size to estimate total";
                          "    number of blocks. This may overestimate total number of blocks, and may";
                          "    show false repair/verify failures when gaps in container are encountered.";
                          "";
                );
                let file_size = file_utils::get_file_size(&param.in_file)?;
                file_size / block_size as u64
            }
        }
    };

    let stats = Arc::new(Mutex::new(Stats::new(
        &ref_block,
        total_block_count,
        json_printer,
    )));

    let mut reader = FileReader::new(
        &param.in_file,
        FileReaderParam {
            write: !param.dry_run,
            buffered: false,
        },
    )?;

    let mut block = Block::dummy();

    let reporter = Arc::new(ProgressReporter::new(
        &stats,
        "SBX block repairing progress",
        "bytes",
        param.pr_verbosity_level,
        param.json_printer.json_enabled(),
    ));

    let header_pred = header_pred_same_ver_uid!(ref_block);

    let mut rs_codec = RSRepairer::new(
        &param.json_printer,
        &ref_block,
        data_par_burst.unwrap().0,
        data_par_burst.unwrap().1,
        data_par_burst.unwrap().2,
    );

    reporter.start();

    json_printer.print_open_bracket(Some("metadata repairs"), BracketType::Square);
    // replace metadata blocks with reference block if broken
    {
        let mut stats = stats.lock().unwrap();

        let mut buffer: [u8; SBX_LARGEST_BLOCK_SIZE] = [0; SBX_LARGEST_BLOCK_SIZE];

        ref_block.sync_to_buffer(None, &mut buffer).unwrap();

        for &p in sbx_block::calc_meta_block_all_write_pos_s(version, data_par_burst).iter() {
            break_if_atomic_bool!(ctrlc_stop_flag);

            reader.seek(SeekFrom::Start(p))?;
            let read_res = reader.read(sbx_block::slice_buf_mut(version, &mut buffer))?;

            let block_broken = read_res.eof_seen
                || match block.sync_from_buffer(&buffer, Some(&header_pred), None) {
                    Ok(()) => false,
                    Err(_) => true,
                };

            if block_broken {
                if json_printer.json_enabled() {
                    if param.verbose {
                        json_printer.print_open_bracket(None, BracketType::Curly);

                        print_maybe_json!(param.json_printer, "seq num : 0");
                        print_maybe_json!(param.json_printer, "pos : {}", p);

                        json_printer.print_close_bracket();
                    }
                } else {
                    print_if!(verbose => param, reporter =>
                              "Replaced invalid metadata block at {} (0x{:X}) with reference block", p, p;);
                }

                stats.blocks_decode_failed += 1;

                reader.seek(SeekFrom::Start(p))?;

                ref_block.sync_to_buffer(None, &mut buffer).unwrap();
                if !param.dry_run {
                    reader.write(sbx_block::slice_buf(version, &buffer))?;
                }

                stats.meta_blocks_repaired += 1;
            } else {
                stats.meta_blocks_decoded += 1;
            }
        }
    }
    json_printer.print_close_bracket();

    if stats.lock().unwrap().meta_blocks_repaired > 0 {
        print_if!(verbose not_json => param, json_printer => "";);
    }

    json_printer.print_open_bracket(Some("data repairs"), BracketType::Square);
    // repair data blocks
    let mut seq_num = 1;
    loop {
        let mut stats = stats.lock().unwrap();

        break_if_atomic_bool!(ctrlc_stop_flag);

        if stats.blocks_so_far() >= total_block_count {
            break;
        }

        let pos = sbx_block::calc_data_block_write_pos(version, seq_num, None, data_par_burst);

        reader.seek(SeekFrom::Start(pos))?;

        let read_res = reader.read(rs_codec.get_block_buffer())?;

        let codec_state = update_rs_codec_and_stats(
            version,
            &header_pred,
            &read_res,
            &mut block,
            seq_num,
            &mut rs_codec,
            &mut stats,
        );

        match codec_state {
            RSCodecState::Ready => {
                repair_blocks_and_update_stats_using_repair_stats(
                    &param,
                    seq_num,
                    &mut rs_codec,
                    &mut stats,
                    &mut reader,
                    &reporter,
                )?;
            }
            RSCodecState::NotReady => {}
        }

        incre_or_break_if_last!(seq_num => seq_num);
    }
    json_printer.print_close_bracket();

    if stats.lock().unwrap().blocks_decode_failed > 0 {
        print_if!(verbose not_json => param, json_printer => "";);
    }

    reporter.stop();

    let stats = stats.lock().unwrap().clone();

    Ok(Some(stats))
}
