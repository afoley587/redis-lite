use crate::prelude::*;

pub struct Aof {
    reader: BufReader<File>,
    writer: BufWriter<File>,
    lock: Mutex<()>,
}

impl Aof {
    pub fn new(path: &str) -> std::io::Result<Self> {
        let write_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;

        let read_file = std::fs::OpenOptions::new().read(true).open(path)?;

        let reader = BufReader::new(read_file);
        let writer = BufWriter::new(write_file);

        Ok(Self {
            reader,
            writer,
            lock: Mutex::new(()),
        })
    }

    pub fn read(&mut self) -> std::io::Result<()> {
        let _lock = self.lock.lock().unwrap();
        loop {
            match read_resp(&mut self.reader) {
                Ok(command) => {
                    println!("Replaying command: {:?}", command);
                    let _ = handle_resp(&command);
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    pub fn write(&mut self, val: &RespValue) -> std::io::Result<()> {
        let _lock = self.lock.lock().unwrap();
        let bytes = marshal(val);
        self.writer.write_all(&bytes)?;
        Ok(())
    }

    pub fn sync(&mut self) -> std::io::Result<()> {
        let _lock = self.lock.lock().unwrap();
        self.writer.flush()?;
        Ok(())
    }
}
