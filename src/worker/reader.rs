use super::super::Error;
use super::super::file_error;
use super::super::FileReader;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::SyncSender;
use std::sync::mpsc::Sender;
use std::sync::mpsc::TrySendError;
use std::time::Duration;
use std::thread;
use std::thread::JoinHandle;

pub fn make_reader(block_size    : usize,
                   write_start   : Option<usize>,
                   write_end_exc : Option<usize>,
                   counter       : &Arc<Mutex<u64>>,
                   shutdown_flag : &Arc<AtomicBool>,
                   in_file       : &str,
                   tx_bytes      : SyncSender<Box<[u8]>>,
                   tx_error      : Sender<Option<Error>>)
                   -> Result<JoinHandle<()>, Error> {
    let write_start = match write_start {
        Some(x) => x,
        None    => 0
    };
    let write_end_exc = match write_end_exc {
        Some(x) => x,
        None    => block_size
    };

    let counter       = Arc::clone(counter);
    let shutdown_flag = Arc::clone(shutdown_flag);
    let reader_res    = file_error::adapt_to_err(FileReader::new(in_file));

    Ok(thread::spawn(move || {
        let mut reader = match reader_res {
            Ok(r)  => r,
            Err(e) => worker_stop!(with_error_ret e => tx_error, shutdown_flag)
        };

        let mut secondary_buf : Option<Box<[u8]>> = None;

        loop {
            worker_stop!(graceful_if_shutdown => tx_error, shutdown_flag);

            // allocate if secondary_buf is empty
            let mut buf = match secondary_buf {
                None    => vec![0; block_size].into_boxed_slice(),
                Some(b) => { secondary_buf = None; b }
            };

            // read into buffer
            let len_read = match reader.read(&mut buf[write_start..write_end_exc]) {
                Ok(l) => l,
                Err(e) => {
                    worker_stop!(with_error file_error::to_err(e) =>
                                 tx_error, shutdown_flag);
                }
            };

            worker_stop!(graceful_if (len_read == 0) => tx_error, shutdown_flag);

            // update stats
            *counter.lock().unwrap() += len_read as u64;

            // send bytes over
            // if full, then put current buffer into secondary buffer and wait
            match tx_bytes.try_send(buf) {
                Ok(()) => {},
                Err(TrySendError::Full(b)) => {
                    secondary_buf = Some(b);
                    thread::sleep(Duration::from_millis(10)); },
                Err(TrySendError::Disconnected(_)) => panic!()
            }
        }
    }))
}