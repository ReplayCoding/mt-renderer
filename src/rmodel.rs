use bytemuck::{Pod, Zeroable};
use log::debug;
use std::{
    ffi::CStr,
    io::{Read, Seek},
};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
struct MtVector3 {
    x: f32,
    y: f32,
    z: f32,
    pad_: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
struct MtAABB {
    minpos: MtVector3,
    maxpos: MtVector3,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
struct MtFloat3A {
    x: f32,
    y: f32,
    z: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
struct MtSphere {
    pos: MtFloat3A,
    r: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
struct MODEL_INFO {
    middist: i32,
    lowdist: i32,
    light_group: u32,
    memory: u16,
    reserved: u16,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
struct ModelHdr {
    magic: u32,
    version: u16,
    jnt_num: u16,
    primitive_num: u16,
    material_num: u16,
    vertex_num: u32,
    index_num: u32,
    polygon_num: u32,
    vertexbuf_size: u32,
    texture_num: u32,
    parts_num: u32,
    padding1: u32,
    joint_info: u64,
    parts_info: u64,
    material_info: u64,
    primitive_info: u64,
    vertex_data: u64,
    index_data: u64,
    rcn_data: u64,
    bounding_sphere: MtSphere,
    bounding_box: MtAABB,
    modelinfo: MODEL_INFO,
}

#[repr(u32)]
#[derive(strum::FromRepr, Debug)]
pub enum PrimitiveTopology {
    TriangleStrip = 4,
}

impl PrimitiveTopology {
    pub fn to_wgpu(&self) -> wgpu::PrimitiveTopology {
        match self {
            PrimitiveTopology::TriangleStrip => wgpu::PrimitiveTopology::TriangleStrip,
        }
    }
}

#[repr(C, packed)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
pub struct PrimitiveInfo {
    // u32 draw_mode:16;
    // u32 vertex_num:16;
    drawmode_vertexnum: u32,
    // u32 parts_no:12;
    // u32 material_no:12;
    // u32 lod:8;
    parts_material_lod: u32,

    // u32 disp:1;
    // u32 shape:1;
    // u32 sort:1;
    // u32 weight_num:5;
    // u32 alphapri:8;
    // u32 vertex_stride:8;
    // u32 topology:6;
    // u32 binormal_flip:1;
    // u32 bridge:1;
    very_large_bitfield: u32,

    vertex_ofs: u32,
    vertex_base: u32,
    inputlayout: u32, // SO_HANDLE
    index_ofs: u32,
    index_num: u32,
    index_base: u32,
    // u32 envelope:8;
    // u32 boundary_num:8;
    // u32 connect_id:16;
    envelope_boundary_connect: u32,
    // u32 min_index:16;
    // u32 max_index:16;
    min_max_index: u32,

    padding_: u32, // pointers are aligned to 8 bytes
    boundary: u64, // struct BOUNDARY_INFO *
}

impl PrimitiveInfo {
    pub fn vertex_stride(&self) -> u32 {
        (self.very_large_bitfield >> 16) & 0xFF
    }

    pub fn parts_no(&self) -> u32 {
        self.parts_material_lod & 0xFFF
    }

    pub fn material_no(&self) -> u32 {
        (self.parts_material_lod >> 12) & 0xFFF
    }

    pub fn inputlayout(&self) -> u32 {
        self.inputlayout
    }

    pub fn vertex_base(&self) -> u32 {
        self.vertex_base
    }

    pub fn index_ofs(&self) -> u32 {
        self.index_ofs
    }

    pub fn index_base(&self) -> u32 {
        self.index_base
    }

    pub fn index_num(&self) -> u32 {
        self.index_num
    }

    pub fn raw_topology(&self) -> u32 {
        (self.very_large_bitfield >> 24) & 0x3f
    }

    pub fn topology(&self) -> PrimitiveTopology {
        PrimitiveTopology::from_repr(self.raw_topology()).unwrap()
    }
}

pub struct ModelFile {
    material_names: Vec<String>,
    primitives: Vec<PrimitiveInfo>,
    vertex_buf: Vec<u8>,
    index_buf: Vec<u16>,
}

impl ModelFile {
    pub fn new<R: Read + Seek>(reader: &mut R) -> anyhow::Result<ModelFile> {
        let mut header_bytes: [u8; 0xa0] = [0; 0xa0];
        reader.read_exact(&mut header_bytes)?;

        let header: &ModelHdr = bytemuck::try_from_bytes(&header_bytes).unwrap();

        debug!("model header: {:#?}", header);

        let mut material_bytes = vec![0u8; header.material_num as usize * 128];
        reader.seek(std::io::SeekFrom::Start(header.material_info as u64))?;
        reader.read_exact(&mut material_bytes)?;
        let material_names: Vec<String> = (0..header.material_num as usize)
            .map(|material_idx| {
                let material_name_bytes =
                    &material_bytes[material_idx * 128..(material_idx + 1) * 128];
                let material_name = CStr::from_bytes_until_nul(material_name_bytes)
                    .unwrap()
                    .to_string_lossy();

                material_name.to_string()
            })
            .collect();

        debug!("materials: {:?}", material_names);

        let mut primitive_arr_bytes = vec![0u8; header.primitive_num as usize * 0x38];
        reader.seek(std::io::SeekFrom::Start(header.primitive_info as u64))?;
        reader.read_exact(&mut primitive_arr_bytes)?;
        let primitives: Vec<PrimitiveInfo> = (0..header.primitive_num as usize)
            .map(|primitive_idx| {
                let primitive_bytes =
                    &primitive_arr_bytes[primitive_idx * 0x38..(primitive_idx + 1) * 0x38];
                let primitive: &PrimitiveInfo = bytemuck::try_from_bytes(primitive_bytes).unwrap();

                debug!(
                    "primitive {}: stride {} (mat {}: {}) layout {}",
                    primitive_idx,
                    primitive.vertex_stride(),
                    primitive.material_no() as usize,
                    &material_names[primitive.material_no() as usize],
                    (primitive.inputlayout() & 0xfffff000) >> 0xc
                );
                *primitive
            })
            .collect();

        let mut vertex_buf = vec![0u8; header.vertexbuf_size as usize];
        reader.seek(std::io::SeekFrom::Start(header.vertex_data))?;
        reader.read_exact(&mut vertex_buf)?;

        let mut index_buf = vec![0u16; header.index_num as usize];
        reader.seek(std::io::SeekFrom::Start(header.index_data))?;
        reader.read_exact(bytemuck::cast_slice_mut(&mut index_buf))?;

        Ok(Self {
            material_names,
            primitives,
            vertex_buf,
            index_buf,
        })
    }

    pub fn index_buf(&self) -> &[u16] {
        &self.index_buf
    }

    pub fn vertex_buf(&self) -> &[u8] {
        &self.vertex_buf
    }

    pub fn primitives(&self) -> &[PrimitiveInfo] {
        &self.primitives
    }

    pub fn material_names(&self) -> &[String] {
        &self.material_names
    }
}

#[test]
fn test_struct_sizes() {
    assert_eq!(std::mem::size_of::<ModelHdr>(), 0xa0);
    assert_eq!(std::mem::size_of::<PrimitiveInfo>(), 0x38);
}
