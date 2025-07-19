mod persistence;
mod resp;
mod store;

mod prelude {
    pub use crate::persistence::*;
    pub use crate::resp::*;
    pub use crate::store::*;
    pub use clap::Parser;
    pub use once_cell::sync::Lazy;
    pub use std::{
        collections::HashMap,
        env,
        fs::File,
        io::{BufReader, BufWriter, prelude::*},
        net::{TcpListener, TcpStream},
        process,
        sync::{Arc, Mutex, RwLock},
        thread,
        time::Duration,
    };
}

use prelude::*;

#[derive(Parser)]
#[command(author, version, about)]
struct Args {
    #[arg(default_value = "0.0.0.0:6379")]
    addr: String,

    #[arg(default_value = "/tmp/aof.log")]
    aof_path: String,
}

fn main() {
    let args = Args::parse();

    let listener = TcpListener::bind(args.addr).unwrap();

    let aof = Arc::new(Mutex::new(
        Aof::new(args.aof_path.as_str(), 1).expect("Failed to open AOF"),
    ));

    let aof_clone = Arc::clone(&aof);
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_secs(1));
            if let Ok(mut aof) = aof_clone.lock() {
                if let Err(e) = aof.sync() {
                    eprintln!("AOF sync failed: {}", e);
                }
            }
        }
    });

    aof.lock().unwrap().read().expect("Failed to replay AOF");

    for stream in listener.incoming() {
        let stream = stream.unwrap();
        let _aof = Arc::clone(&aof);

        thread::spawn(|| {
            let _ = handle_connection(stream, _aof);
        });
    }
}

fn handle_connection(stream: TcpStream, aof: Arc<Mutex<Aof>>) -> Result<(), std::io::Error> {
    let mut buf_reader = BufReader::new(stream);

    loop {
        let command = read_resp(&mut buf_reader)?;

        let response = handle_resp(&command);

        aof.lock().unwrap().write(&command)?;
        buf_reader
            .get_mut()
            .write_all(marshal(&response).as_ref())
            .unwrap();
    }
}
