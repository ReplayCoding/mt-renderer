use std::{
    collections::HashSet,
    env,
    fs::File,
    io::{BufWriter, Write},
    path::Path,
};

fn build_dti_map() {
    println!("cargo:rerun-if-changed=src/dti.txt");
    let mut out_file = BufWriter::new(
        File::create(Path::new(&env::var_os("OUT_DIR").unwrap()).join("dti_generated.rs")).unwrap(),
    );

    let mut map = phf_codegen::Map::new();

    let dtis = std::fs::read_to_string("src/dti.txt").unwrap();
    // some dti entries are duplicated for some reason. Potentially due to TGAAC
    // having two games packaged into one executable
    let mut handled_entries: HashSet<u32> = HashSet::new();
    for line in dtis.lines() {
        let fields: Vec<&str> = line.split(",").collect();
        let [_address, name, _parent, _vtable, crc, _size] = fields[..].try_into().unwrap();

        let crc = u32::from_str_radix(crc, 16).unwrap();
        let name = format!("\"{}\"", name).leak(); // phf writes out exactly as what provide it for the value
        if !handled_entries.contains(&crc) {
            handled_entries.insert(crc);
            map.entry(crc, name);
        }
    }

    write!(
        &mut out_file,
        "static DTI_MAP: phf::Map<u32, &'static str> = {};",
        map.build()
    )
    .unwrap();
}

fn main() {
    build_dti_map();
}
