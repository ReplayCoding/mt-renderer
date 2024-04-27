use serde::Deserialize;
use std::{
    collections::HashSet,
    env,
    fs::File,
    io::{BufWriter, Write},
    path::Path,
};

fn u64_from_hex_string<'de, D>(d: D) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = Option::<String>::deserialize(d)?;

    Ok(s.map(|s| {
        let mut s = s.as_str();

        if s.starts_with("0x") {
            s = &s[2..];
        }

        u64::from_str_radix(s, 16).unwrap()
    }))
}

#[derive(Deserialize)]
#[allow(unused)]
struct DTIEntry {
    #[serde(deserialize_with = "u64_from_hex_string")]
    addr: Option<u64>,
    #[serde(deserialize_with = "u64_from_hex_string")]
    vtable: Option<u64>,
    name: String,
    parent_name: Option<String>,
    crc: u32,
    #[serde(deserialize_with = "u64_from_hex_string")]
    size: Option<u64>,
    file_ext: Option<String>,
}

fn create_clean_name(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            ':' | '<' | '>' => '_',
            _ => c,
        })
        .collect()
}

fn build_dti_map() {
    println!("cargo:rerun-if-changed=src/dti.txt");
    let mut out_file = BufWriter::new(
        File::create(Path::new(&env::var_os("OUT_DIR").unwrap()).join("dti_generated.rs")).unwrap(),
    );

    let mut map = phf_codegen::Map::new();

    let dtis = std::fs::read_to_string("src/dti.txt").unwrap();
    // some dti entries are duplicated for some reason. potentially due to TGAAC
    // having two games packaged into one executable
    let mut handled_entries: HashSet<u32> = HashSet::new();

    writeln!(&mut out_file, "#[allow(non_upper_case_globals)]").unwrap();
    writeln!(&mut out_file, "pub mod generated {{").unwrap();
    for line in dtis.lines() {
        let entry: DTIEntry = serde_json::from_str(line).unwrap();
        if !handled_entries.contains(&entry.crc) {
            handled_entries.insert(entry.crc);

            let clean_name = create_clean_name(&entry.name);

            writeln!(
                &mut out_file,
                "pub const {}: super::DTI = super::DTI {{ name: {:?}, hash: {}, file_ext: {:?} }};",
                clean_name, &entry.name, entry.crc, entry.file_ext
            )
            .unwrap();

            let formatted_entry = format!("{}", clean_name);

            map.entry(entry.crc, &formatted_entry);
        }
    }

    write!(
        &mut out_file,
        "pub(super) const DTI_MAP: phf::Map<u32, super::DTI> = {};",
        map.build()
    )
    .unwrap();

    writeln!(&mut out_file, "}}").unwrap();
}

fn main() {
    build_dti_map();
}
