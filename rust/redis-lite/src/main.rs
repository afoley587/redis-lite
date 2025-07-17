use std::{
    io::{BufReader, prelude::*},
    net::{TcpListener, TcpStream},
    thread,
};

fn main() {
    let listener = TcpListener::bind("0.0.0.0:6379").unwrap();

    for stream in listener.incoming() {
        let stream = stream.unwrap();

        thread::spawn(|| {
            let _ = handle_connection(stream);
        });
    }
}

fn handle_connection(stream: TcpStream) -> Result<(), std::io::Error> {
    println!("Stream opened");
    let mut buf_reader = BufReader::new(stream);

    println!("Stream read");

    loop {
        let command = read_resp(&mut buf_reader)?;
        println!("{:?}", command);

        let response = "+OK\r\n";
        buf_reader.get_mut().write_all(response.as_bytes()).unwrap();
    }
}

fn read_resp<R: BufRead>(reader: &mut R) -> Result<Vec<String>, std::io::Error> {
    let mut line = String::new();
    reader.read_line(&mut line)?; // Read the first line

    println!("{:?}", line);
    if !line.starts_with('*') {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Not a RESP array",
        ));
    }

    let array_len: usize = line[1..].trim().parse().unwrap();
    let mut result = Vec::with_capacity(array_len);

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
        result.push(s);
    }

    Ok(result)
}
