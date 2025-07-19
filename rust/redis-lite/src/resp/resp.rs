use crate::prelude::*;

#[derive(Debug, Clone, PartialEq)]
pub enum RespValue {
    SimpleString(String),
    Error(String),
    Integer(i32),
    BulkString(Option<String>),
    Array(Vec<RespValue>),
    Null,
}

pub fn marshal(value: &RespValue) -> Vec<u8> {
    match value {
        RespValue::SimpleString(s) => format!("+{}\r\n", s).into_bytes(),
        RespValue::Error(s) => format!("-{}\r\n", s).into_bytes(),
        RespValue::Integer(i) => format!(":{}\r\n", i).into_bytes(),
        RespValue::BulkString(Some(s)) => format!("${}\r\n{}\r\n", s.len(), s).into_bytes(),
        RespValue::BulkString(None) => b"$-1\r\n".to_vec(),
        RespValue::Array(arr) => {
            let mut buf = format!("*{}\r\n", arr.len()).into_bytes();
            for item in arr {
                buf.extend(marshal(item));
            }
            buf
        }
        RespValue::Null => b"$-1\r\n".to_vec(),
    }
}

pub fn read_resp<R: BufRead>(reader: &mut R) -> Result<RespValue, std::io::Error> {
    let mut line = String::new();

    // Skip empty or whitespace-only lines
    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line)?;
        if bytes_read == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "EOF",
            ));
        }
        if !line.trim().is_empty() {
            break;
        }
    }

    if !line.starts_with('*') {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Not a RESP array",
        ));
    }
    let array_len: usize = line[1..].trim().parse().unwrap();
    let mut elements = Vec::with_capacity(array_len);

    for _ in 0..array_len {
        line.clear();
        reader.read_line(&mut line)?;
        if !line.starts_with('$') {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Expected bulk string",
            ));
        }

        let str_len: usize = line[1..].trim().parse().unwrap();
        let mut buf = vec![0; str_len + 2];
        reader.read_exact(&mut buf)?;

        let s = String::from_utf8_lossy(&buf[..str_len]).to_string();
        elements.push(RespValue::BulkString(Some(s)));
    }

    Ok(RespValue::Array(elements))
}
