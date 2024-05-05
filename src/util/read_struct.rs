use std::{io::Read, mem::size_of};

use anyhow::anyhow;
use zerocopy::FromBytes;

pub fn read_struct<S, R>(reader: &mut R) -> anyhow::Result<S>
where
    R: Read,
    S: FromBytes,
    [u8; std::mem::size_of::<S>()]:, // wtf
{
    let mut bytes = [0u8; std::mem::size_of::<S>()];
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
