use crate::prelude::*;

static CACHE: Lazy<RwLock<HashMap<String, RespValue>>> = Lazy::new(|| RwLock::new(HashMap::new()));

fn ping(args: Vec<RespValue>) -> RespValue {
    if args.is_empty() {
        RespValue::SimpleString("PONG".to_string())
    } else {
        match &args[0] {
            RespValue::BulkString(Some(s)) => RespValue::SimpleString(s.clone()),
            _ => RespValue::Error("Invalid PING argument".to_string()),
        }
    }
}

fn get(args: Vec<RespValue>) -> RespValue {
    let key = match args.get(0) {
        Some(RespValue::BulkString(Some(k))) => k,
        _ => return RespValue::Error("Missing key for GET".to_string()),
    };

    let map = CACHE.read().unwrap();
    match map.get(key) {
        Some(val) => val.clone(),
        None => RespValue::Null,
    }
}

fn set(args: Vec<RespValue>) -> RespValue {
    if args.len() < 2 {
        return RespValue::Error("SET requires key and value".to_string());
    }

    let key = match &args[0] {
        RespValue::BulkString(Some(k)) => k.clone(),
        _ => return RespValue::Error("Invalid key for SET".to_string()),
    };

    let val = args[1].clone();
    let mut map = CACHE.write().unwrap();
    map.insert(key, val);

    RespValue::SimpleString("OK".to_string())
}

fn del(args: Vec<RespValue>) -> RespValue {
    let mut deleted = 0;
    let mut map = CACHE.write().unwrap();

    for arg in args {
        if let RespValue::BulkString(Some(k)) = arg {
            if map.remove(&k).is_some() {
                deleted += 1;
            }
        }
    }

    RespValue::Integer(deleted)
}

pub fn handle_resp(command: &RespValue) -> RespValue {
    let arr = match command {
        RespValue::Array(a) => a,
        _ => return RespValue::Error("Only arrays accepted.".to_string()),
    };

    let cmd = match arr.get(0) {
        Some(RespValue::BulkString(Some(s))) => s.to_lowercase(),
        _ => return RespValue::Error("Bulk string command expected".to_string()),
    };

    let args = arr[1..].to_vec();

    match cmd.as_str() {
        "ping" => ping(args),
        "get" => get(args),
        "set" => set(args),
        "del" => del(args),
        _ => RespValue::Error("Invalid command".to_string()),
    }
}

import tracemalloc
tracemalloc.start()

class F:
    def __init__(self):
        self.l = list(range(10_000_000))

LEAK_REGISTRY = []
        
def lets_leak():
    
    f = F()
    LEAK_REGISTRY.append(f)

lets_leak()

snapshot = tracemalloc.take_snapshot()
top_stats = snapshot.statistics('lineno')

print("[ Top 10 ]")
for stat in top_stats[:10]:
    print(stat)