use std::io::Read;

pub fn read_struct<S, R>(reader: &mut R) -> std::io::Result<S>
where
    R: Read,
    S: bytemuck::Pod,
    [u8; std::mem::size_of::<S>()]:, // wtf
{
    let mut bytes = [0u8; std::mem::size_of::<S>()];
    reader.read_exact(&mut bytes)?;

    // Copy the struct, so it doesn't reference local variables
    Ok(*bytemuck::from_bytes::<S>(&bytes))
}
