macro_rules! unwrap_or {
    (
        $val:expr, $or:expr
    ) => {{
        match $val {
            Some(x) => x,
            None    => $or
        }
    }}
}

macro_rules! get_ref_block {
    (
        $in_file:expr, $no_meta:expr, $verbose:expr, $pr_verbosity_level:expr
    ) => {{
        let (ref_block_pos, ref_block) =
            match block_utils::get_ref_block(&$in_file,
                                             $no_meta,
                                             $pr_verbosity_level)? {
                None => { return Err(Error::with_message("Failed to find reference block")); },
                Some(x) => x,
            };

        if $verbose {
            println!();
            report_ref_block_info(ref_block_pos, &ref_block);
            println!();
        }

        (ref_block_pos, ref_block)
    }}
}

macro_rules! print_block {
    (
        $(
            $($arg:expr),*;
        )*
    ) => {{
        $( println!($($arg),*) );*
    }}
}

macro_rules! get_RSD_from_ref_block {
    (
        $ref_block_pos:expr, $ref_block:expr, $purpose:expr
    ) => {{
        let ver_usize = ver_to_usize($ref_block.get_version());
        match $ref_block.get_RSD().unwrap() {
            None    => {
                return Err(Error::with_message(&format!("Reference block at byte {} (0x{:X}) is a metadata block but does not have RSD field(must be present to {} for version {})",
                                                        $ref_block_pos,
                                                        $ref_block_pos,
                                                        $purpose,
                                                        ver_usize)));
            },
            Some(x) => x as usize,
        }
    }}
}

macro_rules! get_RSP_from_ref_block {
    (
        $ref_block_pos:expr, $ref_block:expr, $purpose:expr
    ) => {{
        let ver_usize = ver_to_usize($ref_block.get_version());
        match $ref_block.get_RSP().unwrap() {
            None    => {
                return Err(Error::with_message(&format!("Reference block at byte {} (0x{:X}) is a metadata block but does not have RSP field({} for version {})",
                                                        $ref_block_pos,
                                                        $ref_block_pos,
                                                        $purpose,
                                                        ver_usize)));
            },
            Some(x) => x as usize,
        }
    }}
}

macro_rules! return_if_not_ver_uses_rs {
    (
        $version:expr
    ) => {{
        use sbx_specs::*;
        if !ver_uses_rs($version) {
            println!("Version {} does not use Reed-Solomon erasure code, exiting now", ver_to_usize($version));
            return Ok(None);
        }
    }}
}

macro_rules! return_if_ref_not_meta {
    (
        $ref_block_pos:expr, $ref_block:expr, $purpose:expr
    ) => {{
        if $ref_block.is_data() {
            let ver_usize = ver_to_usize($ref_block.get_version());
            return Err(Error::with_message(&format!("Reference block at byte {} (0x{:X}) is not a metadata block(metadata block must be used to {} for version {})",
                                                    $ref_block_pos,
                                                    $ref_block_pos,
                                                    $purpose,
                                                    ver_usize)));
        }
    }}
}

macro_rules! get_burst_or_guess {
    (
        $in_file:expr, $ref_block_pos:expr, $ref_block:expr, $burst:expr
    ) => {{
        unwrap_or!($burst,
                   if ver_uses_rs($ref_block.get_version()) {
                       unwrap_or!(block_utils::guess_burst_err_resistance_level(&$in_file,
                                                                                $ref_block_pos,
                                                                                &$ref_block)?,
                                  {
                                      return Err(
                                          Error::with_message(
                                              "Failed to guess burst resistance level, please specify via --burst option"));
                                  })
                   } else {
                       0
                   })
    }}
}

macro_rules! print_if_verbose {
    (
        $param:expr, $reporter:expr =>
            $(
                $($expr:expr),*;
            )*
    ) => {{
        if $param.verbose {
            pause_reporter!($reporter => $($($expr),*;)*);
        }
    }}
}

macro_rules! pause_reporter {
    (
        $reporter:expr =>
            $(
                $($expr:expr),*;
            )*
    ) => {{
        $reporter.pause();
        print_block!($($($expr),*;)*);
        $reporter.resume();
    }}
}