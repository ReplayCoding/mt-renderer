const DEFAULT_CHUNK_SIZE: usize = 16;

pub fn hexdump_custom(data: &[u8], chunk_size: usize) -> String {
    let mut s = "".to_string();

    for chunk in data.chunks(chunk_size) {
        for b in chunk {
            s.push_str(&format!("{:02x} ", b));
        }

        // padding to align the ascii display
        for _ in 0..chunk_size - chunk.len() {
            s.push_str("   ");
        }

        s.push_str(&" | ");
        for b in chunk {
            let c = *b as char;
            if c.is_ascii_alphanumeric() {
                s.push(c);
            } else {
                s.push('.');
            }
        }

        s.push('\n');
    }

    s
}

pub fn hexdump(data: &[u8]) -> String {
    hexdump_custom(data, DEFAULT_CHUNK_SIZE)
}
