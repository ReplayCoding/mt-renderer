use std::{ffi::CStr, io::Read, mem::size_of};

use anyhow::anyhow;
use encoding_rs::SHIFT_JIS;
use zerocopy::FromBytes;

pub fn read_struct<S, R>(reader: &mut R) -> anyhow::Result<S>
where
    R: Read,
    S: FromBytes,
    // [u8; std::mem::size_of::<S>()]:, // wtf
{
    // TOOD: make this a slice when that stupid feature (generic_const_exprs) gets stabilized
    let mut bytes = vec![0u8; std::mem::size_of::<S>()];
    reader.read_exact(&mut bytes)?;

    S::read_from(&bytes).ok_or_else(|| anyhow!("couldn't read struct!"))
}

pub fn read_struct_array<'a, S>(
    bytes: &'a [u8],
    num_structs: usize,
) -> anyhow::Result<impl Iterator<Item = Option<&'a S>>>
where
    S: FromBytes + 'a,
{
    if bytes.len() < (num_structs * size_of::<S>()) {
        return Err(anyhow!(
            "not enough bytes to read array: {} < ({} * {})",
            bytes.len(),
            num_structs,
            size_of::<S>()
        ));
    }

    Ok((0..num_structs).map(|idx| {
        let offs = idx * size_of::<S>();
        let bytes = &bytes[offs..offs + size_of::<S>()];

        S::ref_from(bytes)
    }))
}

pub fn read_struct_array_stream<S, R>(reader: &mut R, num_structs: usize) -> anyhow::Result<Vec<S>>
where
    S: FromBytes + Clone,
    R: Read,
{
    let mut bytes = vec![0u8; std::mem::size_of::<S>() * num_structs];
    reader.read_exact(&mut bytes)?;

    let mut v = vec![];
    for obj in read_struct_array::<S>(&bytes, num_structs)? {
        v.push(obj.ok_or_else(|| anyhow!("should not be None"))?.clone())
    }

    Ok(v)
}

pub fn read_null_terminated_string<R: Read>(
    reader: &mut R,
    max_size: usize,
) -> anyhow::Result<String> {
    let mut v = vec![];

    let mut num_read_bytes = 0;
    let mut current_byte = [0u8; 1];
    while let Ok(_) = reader.read_exact(&mut current_byte) {
        num_read_bytes += 1;
        let current_byte = current_byte[0];

        // TODO: is this correct?
        if current_byte == 0 || num_read_bytes > max_size {
            v.push(0);
            break;
        }

        v.push(current_byte);
    }

    let bytes = CStr::from_bytes_until_nul(&v)?;

    let (decoded, _, _) = SHIFT_JIS.decode(&bytes.to_bytes());

    Ok(decoded.to_string())
}
