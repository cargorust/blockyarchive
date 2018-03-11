mod header;
mod metadata;
mod crc;
mod tests;
mod header_tests;
mod metadata_tests;

use self::header::Header;
pub use self::metadata::Metadata;
pub use self::metadata::MetadataID;
use super::smallvec::SmallVec;

use super::sbx_specs::{Version,
                       SBX_HEADER_SIZE,
                       SBX_FILE_UID_LEN,
                       SBX_FIRST_DATA_SEQ_NUM,
                       ver_to_block_size,
                       ver_uses_rs};
use self::crc::*;

use super::multihash;

macro_rules! make_meta_getter {
    (
        $func_name:ident => $meta_id:ident => $ret_type:ty
    ) => {
        #[allow(non_snake_case)]
        pub fn $func_name (&self) -> Result<Option<$ret_type>, Error> {
            match self.get_meta_ref_by_id(MetadataID::$meta_id)? {
                None                             => Ok(None),
                Some(&Metadata::$meta_id(ref x)) => Ok(Some(x.clone())),
                _                                => panic!(),
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BlockType {
    Data, Meta
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Error {
    IncorrectBlockType,
    InconsistentHeaderBlockType,
    InsufficientBufferSize,
    IncorrectBufferSize,
    TooMuchMetaData,
    InvalidCRC,
    SeqNumOverflow,
    ParseError,
    FailedPred,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Data {
    Data,
    Meta(Vec<Metadata>)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    header      : Header,
    data        : Data,
}

macro_rules! slice_buf {
    (
        whole => $self:ident, $buf:ident
    ) => {
        &$buf[..block_size!($self)]
    };
    (
        whole_mut => $self:ident, $buf:ident
    ) => {
        &mut $buf[..block_size!($self)]
    };
    (
        header => $self:ident, $buf:ident
    ) => {
        &$buf[..SBX_HEADER_SIZE]
    };
    (
        header_mut => $self:ident, $buf:ident
    ) => {
        &mut $buf[..SBX_HEADER_SIZE]
    };
    (
        data => $self:ident, $buf:ident
    ) => {
        &$buf[SBX_HEADER_SIZE..block_size!($self)]
    };
    (
        data_mut => $self:ident, $buf:ident
    ) => {
        &mut $buf[SBX_HEADER_SIZE..block_size!($self)]
    }
}

macro_rules! check_buffer {
    (
        $self:ident, $buf:ident
    ) => {
        if $buf.len() < block_size!($self) {
            panic!("Insufficient buffer size");
            //return Err(Error::InsufficientBufferSize);
        }
    }
}

macro_rules! block_size {
    (
        $self:ident
    ) => {
        ver_to_block_size($self.header.version)
    }
}

/*macro_rules! data_size {
    (
        $self:ident
    ) => {
        ver_to_data_size($self.header.version)
    }
}*/

pub fn write_padding(version : Version,
                     skip    : usize,
                     buffer  : &mut [u8]) -> usize {
    let block_size = ver_to_block_size(version);
    let start      = SBX_HEADER_SIZE + skip;

    for i in start..block_size {
        buffer[i] = 0x1A;
    }

    block_size - start
}

pub fn slice_buf(version : Version,
                 buffer  : & [u8]) -> & [u8] {
    &buffer[..ver_to_block_size(version)]
}

pub fn slice_buf_mut(version : Version,
                     buffer  : &mut [u8]) -> &mut [u8] {
    &mut buffer[..ver_to_block_size(version)]
}

pub fn slice_header_buf(buffer  : &[u8]) -> &[u8] {
    &buffer[..SBX_HEADER_SIZE]
}

pub fn slice_header_buf_mut(buffer  : &mut [u8]) -> &mut [u8] {
    &mut buffer[..SBX_HEADER_SIZE]
}

pub fn slice_data_buf(version : Version,
                      buffer  : &[u8]) -> &[u8] {
    &buffer[SBX_HEADER_SIZE..ver_to_block_size(version)]
}

pub fn slice_data_buf_mut(version : Version,
                          buffer  : &mut [u8]) -> &mut [u8] {
    &mut buffer[SBX_HEADER_SIZE..ver_to_block_size(version)]
}

pub fn check_if_buffer_valid(buffer : &[u8]) -> bool {
    let mut block = Block::new(Version::V1,
                               b"\x00\x00\x00\x00\x00\x00",
                               BlockType::Data);

    match block.sync_from_buffer(buffer, None) {
        Ok(()) => {},
        Err(_) => { return false; }
    }

    block.verify_crc(buffer).unwrap()
}

pub fn seq_num_is_parity(seq_num       : u32,
                         data_shards   : usize,
                         parity_shards : usize) -> bool {
    if        seq_num == 0 {
        false // this is metadata block
    } else {  // data sets
        let index        = seq_num - SBX_FIRST_DATA_SEQ_NUM as u32;
        let index_in_set = index % (data_shards + parity_shards) as u32;

        (data_shards as u32 <= index_in_set)
    }
}

pub fn calc_rs_enabled_meta_dup_write_pos_s(version              : Version,
                                            parity_shards        : usize,
                                            burst_err_resistance : usize)
                                            -> SmallVec<[u64; 32]> {
    let block_size = ver_to_block_size(version) as u64;

    let mut res =
        calc_rs_enabled_meta_dup_write_indices(parity_shards,
                                               burst_err_resistance);

    for i in res.iter_mut() {
        *i = *i * block_size;
    }

    res
}

pub fn calc_rs_enabled_meta_dup_write_indices(parity_shards        : usize,
                                              burst_err_resistance : usize)
                                              -> SmallVec<[u64; 32]> {
    let mut res : SmallVec<[u64; 32]> =
        SmallVec::with_capacity(1 + parity_shards);

    for i in 1..1 + parity_shards as u64 {
        res.push(i * (1 + burst_err_resistance) as u64);
    }

    res
}

pub fn calc_rs_enabled_meta_dup_write_all_s(version              : Version,
                                            parity_shards        : usize,
                                            burst_err_resistance : usize)
                                            -> SmallVec<[u64; 32]> {
    let mut res = calc_rs_enabled_meta_dup_write_pos_s(version,
                                                       parity_shards,
                                                       burst_err_resistance);

    res.push(0);

    res
}

pub fn calc_rs_enabled_meta_all_write_indices(parity_shards        : usize,
                                              burst_err_resistance : usize)
                                              -> SmallVec<[u64; 32]> {
    let mut res = calc_rs_enabled_meta_dup_write_indices(parity_shards,
                                                         burst_err_resistance);

    res.push(0);

    res
}

pub fn calc_rs_enabled_data_write_pos(seq_num              : u32,
                                      version              : Version,
                                      data_shards          : usize,
                                      parity_shards        : usize,
                                      burst_err_resistance : usize) -> u64 {
    let block_size = ver_to_block_size(version) as u64;

    calc_rs_enabled_data_write_index(seq_num,
                                     data_shards,
                                     parity_shards,
                                     burst_err_resistance) * block_size
}

pub fn calc_rs_enabled_data_write_index(seq_num              : u32,
                                        data_shards          : usize,
                                        parity_shards        : usize,
                                        burst_err_resistance : usize) -> u64 {
    // the following transforms seq num to data index
    // then do the transformation based on data index
    assert!(seq_num >= SBX_FIRST_DATA_SEQ_NUM as u32);

    if burst_err_resistance == 0 {
        return parity_shards as u64 + seq_num as u64;
    }

    // calculate data index
    let index = seq_num as u64 - SBX_FIRST_DATA_SEQ_NUM as u64;

    let data_shards          = data_shards      as u64;
    let parity_shards        = parity_shards    as u64;
    let burst_err_resistance = burst_err_resistance as u64;

    let super_block_set_size = (data_shards + parity_shards) * burst_err_resistance;

    // sub A block sets partitioning deals with the super block set
    // of the sequential data index arrangement
    // i.e. sub A = partitioning of input
    //
    // sub B block sets partitioning deals with the super block set
    // of the interleaving data index arrangement
    // i.e. sub B = partitioning of output
    //
    // sub A block set partitioning slices at total shards interval
    // sub B block set partitioning slices at burst resistance level interval
    let sub_a_block_set_size = data_shards + parity_shards;
    let sub_b_block_set_size = burst_err_resistance;

    // calculate the index of the start of super block set with
    // respect to the data index
    let super_block_set_index    = index / super_block_set_size;
    // calculate index of current seq num inside the current super block set
    let index_in_super_block_set = index % super_block_set_size;

    // calculate the index of the start of sub A block set inside
    // the current super block set
    let sub_a_block_set_index    = index_in_super_block_set / sub_a_block_set_size;
    // calculate index of current seq num inside the current sub A block set
    let index_in_sub_a_block_set = index_in_super_block_set % sub_a_block_set_size;

    let sub_b_block_set_index    = index_in_sub_a_block_set;
    let index_in_sub_b_block_set = sub_a_block_set_index;

    let new_index_in_super_block_set =
        sub_b_block_set_index * sub_b_block_set_size + index_in_sub_b_block_set;

    // M = data_shards
    // N = parity_shards
    //
    // calculate the number of metadata blocks before the current
    // seq num in the interleaving scheme
    let meta_block_count =
        if super_block_set_index == 0 { // first super block set
            // one metadata block at front of first (1 + N) sub B blocks
            if sub_b_block_set_index < 1 + parity_shards {
                1 + sub_b_block_set_index
            } else {
                1 + parity_shards
            }
        } else {
            1 + parity_shards
        };

    // finally calculate the index in interleaving data index arrangement
    let new_index =
    // number of metadata blocks in front
        meta_block_count

    // index of start of super block set
        + (super_block_set_size * super_block_set_index)

    // index inside the super block set
        + new_index_in_super_block_set;

    new_index
}

pub fn calc_rs_enabled_seq_num_at_index(index                : u64,
                                        data_shards          : usize,
                                        parity_shards        : usize,
                                        burst_err_resistance : usize) -> u32 {
    // the following essentially does the same index transformation in
    // calc_rs_enabled_data_write_index does but in reverse order
    if burst_err_resistance == 0 {
        if index < 1 + parity_shards as u64 {
            return 0;
        } else {
            return (index - parity_shards as u64) as u32;
        }
    }

    let data_shards          = data_shards      as u64;
    let parity_shards        = parity_shards    as u64;
    let burst_err_resistance = burst_err_resistance as u64;

    // handle metadata seq nums first
    if index < (1 + parity_shards) * (1 + burst_err_resistance) {
        if index % (1 + burst_err_resistance) == 0 {
            return 0;
        }
    }

    let meta_block_count =
    // M = data shards
    // N = parity_shards
    // B = burst_err_resistance
    //
    // if index is in first 1 + N block set
        if index < (1 + parity_shards) * (1 + burst_err_resistance) {
            1 + index / (1 + burst_err_resistance)
        } else {
            1 + parity_shards
        };

    let super_block_set_size = (data_shards + parity_shards) * burst_err_resistance;

    let sub_a_block_set_size = data_shards + parity_shards;
    let sub_b_block_set_size = burst_err_resistance;

    let index_without_meta = index - meta_block_count;

    let super_block_set_index    = index_without_meta / super_block_set_size;
    let index_in_super_block_set = index_without_meta % super_block_set_size;

    let sub_b_block_set_index    = index_in_super_block_set / sub_b_block_set_size;
    let index_in_sub_b_block_set = index_in_super_block_set % sub_b_block_set_size;

    let sub_a_block_set_index    = index_in_sub_b_block_set;
    let index_in_sub_a_block_set = sub_b_block_set_index;

    let old_index_in_super_block_set =
        sub_a_block_set_index * sub_a_block_set_size + index_in_sub_a_block_set;

    let old_index =
    // index of start of super block set
        (super_block_set_size * super_block_set_index)

    // index inside the super block set
        + old_index_in_super_block_set;

    (old_index as u32) + SBX_FIRST_DATA_SEQ_NUM as u32
}

impl Block {
    pub fn new(version    : Version,
               uid   : &[u8; SBX_FILE_UID_LEN],
               block_type : BlockType)
               -> Block {
        match block_type {
            BlockType::Data => {
                let seq_num = SBX_FIRST_DATA_SEQ_NUM as u32;
                Block {
                    header : Header::new(version, uid.clone(), seq_num),
                    data   : Data::Data,
                }
            },
            BlockType::Meta => {
                let seq_num = 0 as u32;
                Block {
                    header : Header::new(version, uid.clone(), seq_num),
                    data   : Data::Meta(Vec::with_capacity(10)),
                }
            }
        }
    }

    pub fn dummy() -> Block {
        let version = Version::V1;
        let seq_num = SBX_FIRST_DATA_SEQ_NUM as u32;
        Block {
            header : Header::new(version, [0; 6], seq_num),
            data   : Data::Data,
        }
    }

    pub fn get_version(&self) -> Version {
        self.header.version
    }

    pub fn set_version(&mut self,
                       version : Version) {
        self.header.version = version;
    }

    pub fn get_uid(&self) -> [u8; SBX_FILE_UID_LEN] {
        self.header.uid
    }

    pub fn set_uid(&mut self,
                   uid : [u8; SBX_FILE_UID_LEN]) {
        self.header.uid = uid;
    }

    pub fn get_crc(&self) -> u16 {
        self.header.crc
    }

    pub fn get_seq_num(&self) -> u32 {
        self.header.seq_num
    }

    pub fn set_seq_num(&mut self,
                       seq_num : u32) {
        self.header.seq_num = seq_num;

        self.switch_block_type_to_match_header();
    }

    pub fn add_seq_num(&mut self,
                       val : u32)
                       -> Result<(), Error> {
        match self.header.seq_num.checked_add(val) {
            None    => { return Err(Error::SeqNumOverflow); },
            Some(x) => { self.header.seq_num = x; }
        }

        self.switch_block_type_to_match_header();

        Ok(())
    }

    pub fn add1_seq_num(&mut self)
                        -> Result<(), Error> {
        self.add_seq_num(1)
    }

    pub fn block_type(&self) -> BlockType {
        match self.data {
            Data::Data    => BlockType::Data,
            Data::Meta(_) => BlockType::Meta
        }
    }

    pub fn is_meta(&self) -> bool {
        match self.block_type() {
            BlockType::Data => false,
            BlockType::Meta => true
        }
    }

    pub fn is_data(&self) -> bool {
        match self.block_type() {
            BlockType::Data => true,
            BlockType::Meta => false
        }
    }

    pub fn is_parity(&self,
                     data_shards   : usize,
                     parity_shards : usize) -> bool {
        ver_uses_rs(self.header.version)
            && seq_num_is_parity(self.get_seq_num(),
                                 data_shards,
                                 parity_shards)
    }

    pub fn data_index(&self,
                      dat_par_shards : Option<(usize, usize)>)
                      -> Option<u32> {
        match self.data {
            Data::Meta(_) => None,
            Data::Data    => {
                let data_index =
                    self.header.seq_num
                    - SBX_FIRST_DATA_SEQ_NUM as u32;

                match dat_par_shards {
                    None                               => Some(data_index),
                    Some((data_shards, parity_shards)) => {
                        if self.is_parity(data_shards, parity_shards) {
                            None
                        } else {
                            let set_index =
                                data_index / (data_shards + parity_shards) as u32;
                            let index_in_set =
                                data_index % (data_shards + parity_shards) as u32;

                            Some(set_index * data_shards as u32 + index_in_set)
                        }
                    }
                }
            }
        }
    }

    pub fn get_meta_ref_by_id(&self,
                              id : MetadataID)
                              -> Result<Option<&Metadata>, Error> {
        match self.data {
            Data::Data           => Err(Error::IncorrectBlockType),
            Data::Meta(ref meta) => {
                Ok(metadata::get_meta_ref_by_id(id, meta))
            }
        }
    }

    pub fn get_meta_ref_mut_by_id(&mut self,
                                  id : MetadataID)
                                  -> Result<Option<&mut Metadata>, Error> {
        match self.data {
            Data::Data               => Err(Error::IncorrectBlockType),
            Data::Meta(ref mut meta) => {
                Ok(metadata::get_meta_ref_mut_by_id(id, meta))
            }
        }
    }

    make_meta_getter!(get_FNM => FNM => String);
    make_meta_getter!(get_SNM => SNM => String);
    make_meta_getter!(get_FSZ => FSZ => u64);
    make_meta_getter!(get_FDT => FDT => i64);
    make_meta_getter!(get_SDT => SDT => i64);
    make_meta_getter!(get_HSH => HSH => multihash::HashBytes);
    make_meta_getter!(get_RSD => RSD => u8);
    make_meta_getter!(get_RSP => RSP => u8);

    pub fn meta(&self) -> Result<&Vec<Metadata>, Error> {
        match self.data {
            Data::Data           => Err(Error::IncorrectBlockType),
            Data::Meta(ref meta) => Ok(meta)
        }
    }

    pub fn meta_mut(&mut self) -> Result<&mut Vec<Metadata>, Error> {
        match self.data {
            Data::Data               => Err(Error::IncorrectBlockType),
            Data::Meta(ref mut meta) => Ok(meta)
        }
    }

    pub fn calc_crc(&self, buffer : &[u8]) -> u16 {
        check_buffer!(self, buffer);

        let crc = self.header.calc_crc();

        crc_ccitt_generic(crc, slice_buf!(data => self, buffer))
    }

    pub fn update_crc(&mut self,
                      buffer : &[u8]) {
        self.header.crc = self.calc_crc(buffer);
    }

    fn header_type_matches_block_type(&self) -> bool {
        self.header.header_type() == self.block_type()
    }

    pub fn sync_to_buffer(&mut self,
                          update_crc : Option<bool>,
                          buffer     : &mut [u8])
                          -> Result<(), Error> {
        check_buffer!(self, buffer);

        let update_crc = update_crc.unwrap_or(true);

        match self.data {
            Data::Meta(ref meta) => {
                if self.get_seq_num() == 0 {  // not a metadata parity block
                    // transform metadata to bytes
                    metadata::to_bytes(meta, slice_buf!(data_mut => self, buffer))?;
                }
            },
            Data::Data => {}
        }

        match self.block_type() {
            BlockType::Data => if update_crc { self.update_crc(buffer) },
            BlockType::Meta =>                 self.update_crc(buffer)
        }

        self.header.to_bytes(slice_buf!(header_mut => self, buffer));

        Ok(())
    }

    fn switch_block_type(&mut self) {
        let block_type = self.block_type();

        if block_type == BlockType::Meta {
            self.data = Data::Data;
        } else {
            self.data = Data::Meta(Vec::with_capacity(10));
        }
    }

    fn switch_block_type_to_match_header(&mut self) {
        if !self.header_type_matches_block_type() {
            self.switch_block_type();
        }
    }

    pub fn sync_from_buffer_header_only(&mut self,
                                        buffer : &[u8])
                                        -> Result<(), Error> {
        self.header.from_bytes(slice_buf!(header => self, buffer))?;

        self.switch_block_type_to_match_header();

        Ok(())
    }

    pub fn sync_from_buffer(&mut self,
                            buffer : &[u8],
                            pred   : Option<&Fn(&Block) -> bool>)
                            -> Result<(), Error>
    {
        self.sync_from_buffer_header_only(buffer)?;

        check_buffer!(self, buffer);

        self.enforce_crc(buffer)?;

        match self.data {
            Data::Meta(ref mut meta) => {
                // parse if it is metadata and not parity block
                // or if it is a RS parity block
                if self.header.seq_num == 0
                    || ver_uses_rs(self.header.version) {
                    meta.clear();
                    let res = metadata::from_bytes(slice_buf!(data => self, buffer))?;
                    for r in res.into_iter() {
                        meta.push(r);
                    }
                }
            },
            Data::Data => {}
        }

        match pred {
            Some(pred) =>
                if pred(&self) { Ok(()) } else { Err(Error::FailedPred) },
            None       => Ok(())
        }
    }

    pub fn verify_crc(&self,
                      buffer : &[u8])
                      -> Result<bool, Error> {
        Ok(self.header.crc == self.calc_crc(buffer))
    }

    pub fn enforce_crc(&self,
                       buffer : &[u8])
                       -> Result<(), Error> {
        if self.verify_crc(buffer)? {
            Ok(())
        } else {
            Err(Error::InvalidCRC)
        }
    }
}
