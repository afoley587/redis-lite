use std::{
    collections::HashMap,
    fs::File,
    io::{BufReader, BufWriter, prelude::*},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex, RwLock},
    thread,
    time::Duration,
};

use once_cell::sync::Lazy;

static CACHE: Lazy<RwLock<HashMap<String, RespValue>>> = Lazy::new(|| RwLock::new(HashMap::new()));

const RespString: u8 = b'+';
const RespError: u8 = b'-';
const RespInteger: u8 = b':';
const RespBulk: u8 = b'$';
const RespArray: u8 = b'*';
const RespNull: u8 = b'_';

#[derive(Debug, Clone)]
struct RespValue {
    pub Type: u8,
    pub StringVal: Option<String>,
    pub IntegerVal: Option<i32>,
    pub BulkVal: Option<String>,
    pub ArrayVal: Option<Vec<RespValue>>,
}

pub struct Aof {
    file: File,
    reader: BufReader<File>,
    writer: BufWriter<File>,
    lock: Mutex<()>,
    sync_period: Duration,
}

impl Aof {
    pub fn new(path: &str, sync_period_secs: u64) -> std::io::Result<Self> {
        let write_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;

        let read_file = std::fs::OpenOptions::new().read(true).open(path)?;

        let file = write_file.try_clone()?;

        let reader = BufReader::new(read_file);
        let writer = BufWriter::new(write_file);

        Ok(Self {
            file: file, // optional: store separately if needed
            reader,
            writer,
            lock: Mutex::new(()),
            sync_period: Duration::from_secs(sync_period_secs),
        })
    }

    pub fn read(&mut self) -> std::io::Result<()> {
        let _lock = self.lock.lock().unwrap();
        loop {
            match read_resp(&mut self.reader) {
                Ok(command) => {
                    println!("COMMAND {:?}", command);
                    let _ = handle_resp(&command); // ignore the result, just rebuild state
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    break; // done reading AOF
                }
                Err(e) => return Err(e), // real error
            }
        }

        Ok(())
    }

    pub fn write(&mut self, val: &RespValue) -> std::io::Result<()> {
        let _lock = self.lock.lock().unwrap(); // lock is held while writing

        let bytes = val.marshal(); // assumes marshal() returns Vec<u8>
        self.writer.write_all(&bytes)?;
        self.writer.flush()?; // ensure it's written to the file

        Ok(())
    }
    // pub fn sync() {}
    // pub fn close() {}
}

impl RespValue {
    fn marshal_simple_string(&self) -> Vec<u8> {
        let binding = "".to_string();
        let s = self.StringVal.as_ref().unwrap_or(&binding);
        let mut buf = vec![RespString];
        buf.extend_from_slice(s.as_bytes());
        buf.extend_from_slice(b"\r\n");
        buf
    }

    fn marshal_error(&self) -> Vec<u8> {
        let binding = "ERR".to_string();
        let s = self.StringVal.as_ref().unwrap_or(&binding);
        let mut buf = vec![RespError];
        buf.extend_from_slice(s.as_bytes());
        buf.extend_from_slice(b"\r\n");
        buf
    }

    fn marshal_integer(&self) -> Vec<u8> {
        let i = self.IntegerVal.unwrap_or(0);
        let mut buf = vec![RespInteger];
        buf.extend_from_slice(i.to_string().as_bytes());
        buf.extend_from_slice(b"\r\n");
        buf
    }

    fn marshal_bulk_string(&self) -> Vec<u8> {
        match &self.BulkVal {
            Some(s) => {
                let mut buf = vec![RespBulk];
                buf.extend_from_slice(s.len().to_string().as_bytes());
                buf.extend_from_slice(b"\r\n");
                buf.extend_from_slice(s.as_bytes());
                buf.extend_from_slice(b"\r\n");
                buf
            }
            None => b"$-1\r\n".to_vec(), // Null bulk string
        }
    }

    fn marshal_array(&self) -> Vec<u8> {
        let binding = vec![];
        let items = self.ArrayVal.as_ref().unwrap_or(&binding);
        let mut buf = vec![RespArray];
        buf.extend_from_slice(items.len().to_string().as_bytes());
        buf.extend_from_slice(b"\r\n");
        for item in items {
            buf.extend(item.marshal());
        }
        buf
    }

    fn marshal_null(&self) -> Vec<u8> {
        b"$-1\r\n".to_vec() // or "*-1\r\n" for null array
    }

    pub fn marshal(&self) -> Vec<u8> {
        match self.Type {
            RespString => self.marshal_simple_string(),
            RespError => self.marshal_error(),
            RespInteger => self.marshal_integer(),
            RespBulk => self.marshal_bulk_string(),
            RespArray => self.marshal_array(),
            RespNull => self.marshal_null(),
            _ => b"-ERR unknown type\r\n".to_vec(),
        }
    }
}

fn newSimpleString(val: &str) -> RespValue {
    return RespValue {
        Type: RespString,
        StringVal: Some(String::from(val)),
        IntegerVal: None,
        BulkVal: None,
        ArrayVal: None,
    };
}

fn newNull() -> RespValue {
    return RespValue {
        Type: RespNull,
        StringVal: None,
        IntegerVal: None,
        BulkVal: None,
        ArrayVal: None,
    };
}

fn newError(val: &str) -> RespValue {
    return RespValue {
        Type: RespError,
        StringVal: Some(String::from(val)),
        IntegerVal: None,
        BulkVal: None,
        ArrayVal: None,
    };
}

fn newInt(val: i32) -> RespValue {
    return RespValue {
        Type: RespInteger,
        StringVal: None,
        IntegerVal: Some(val),
        BulkVal: None,
        ArrayVal: None,
    };
}

fn ping(args: Vec<RespValue>) -> RespValue {
    if args.is_empty() {
        return newSimpleString("PONG");
    }
    match &args[0].BulkVal {
        Some(val) => newSimpleString(val),
        None => newError("Invalid PING argument"),
    }
}

fn get(args: Vec<RespValue>) -> RespValue {
    let key = match args.get(0).and_then(|v| v.BulkVal.as_ref()) {
        Some(k) => k,
        None => return newError("Missing key for GET"),
    };

    let map = CACHE.read().unwrap();

    match map.get(key) {
        Some(val) => val.clone(),
        None => newNull(),
    }
}

fn set(args: Vec<RespValue>) -> RespValue {
    if args.len() < 2 {
        return newError("SET requires key and value");
    }

    let key = match args[0].BulkVal.as_ref() {
        Some(k) => k.clone(),
        None => return newError("Invalid key for SET"),
    };

    let val = args[1].clone(); // already a RespValue
    let mut map = CACHE.write().unwrap();
    map.insert(key, val);

    newSimpleString("OK")
}

fn del(args: Vec<RespValue>) -> RespValue {
    let mut deleted = 0;
    let mut map = CACHE.write().unwrap();

    for arg in args.iter() {
        if let Some(k) = &arg.BulkVal {
            if map.remove(k).is_some() {
                deleted += 1;
            }
        }
    }

    newInt(deleted)
}

fn main() {
    let listener = TcpListener::bind("0.0.0.0:6379").unwrap();

    let aof = Arc::new(Mutex::new(
        Aof::new("/tmp/aof.log", 1).expect("Failed to open AOF"),
    ));

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
            .write_all(response.marshal().as_ref())
            .unwrap();
    }
}

fn handle_resp(command: &RespValue) -> RespValue {
    if command.Type != RespArray {
        return newError("Only arrays accepted.");
    }

    let cmd = match command
        .ArrayVal
        .as_ref()
        .and_then(|arr| arr.get(0))
        .and_then(|v| v.BulkVal.as_ref())
        .map(|s| s.to_lowercase())
    {
        Some(c) => c,
        None => return newError("Bulk string expected"),
    };

    let args: Vec<RespValue> = command.ArrayVal.as_ref().unwrap()[1..].to_vec();

    let resp = match cmd.as_str() {
        "ping" => ping(args),
        "get" => get(args),
        "set" => set(args),
        "del" => del(args),
        _ => newError("Invalid command"),
    };

    resp
}

fn read_resp<R: BufRead>(reader: &mut R) -> Result<RespValue, std::io::Error> {
    let mut line = String::new();
    reader.read_line(&mut line)?; // Read the first line

    println!("Line is {:?}", line);
    if !line.starts_with('*') {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Not a RESP array",
        ));
    }

    let array_len: usize = line[1..].trim().parse().unwrap();

    let mut resp = RespValue {
        Type: RespArray,
        StringVal: None,
        IntegerVal: None,
        BulkVal: None,
        ArrayVal: Some(Vec::new()),
    };

    for _ in 0..array_len {
        line.clear();
        reader.read_line(&mut line)?; // Should be $<len>\r\n
        if !line.starts_with('$') {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Expected bulk string",
            ));
        }

        let str_len: usize = line[1..].trim().parse().unwrap();

        let mut buf = vec![0; str_len + 2]; // +2 for \r\n
        reader.read_exact(&mut buf)?;

        let s = String::from_utf8_lossy(&buf[..str_len]).to_string();

        let resp_item = RespValue {
            Type: RespBulk,
            StringVal: None,
            IntegerVal: None,
            BulkVal: Some(s.clone()),
            ArrayVal: None,
        };

        resp.ArrayVal
            .as_mut()
            .expect("ArrayVal should be Some")
            .push(resp_item);
    }

    Ok(resp)
}
