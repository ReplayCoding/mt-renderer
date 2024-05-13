use anyhow::anyhow;
use log::{debug, trace};
use std::{
    ffi::CStr,
    io::{Read, Seek},
    mem::size_of,
};
use zerocopy::{AsBytes, FromBytes, FromZeroes};

use crate::util;

#[repr(C, packed)]
#[derive(FromBytes, FromZeroes, Debug, Copy, Clone)]
struct MtVector3 {
    x: f32,
    y: f32,
    z: f32,
    pad_: f32,
}

#[repr(C, packed)]
#[derive(FromBytes, FromZeroes, Debug, Copy, Clone)]
struct MtVector4 {
    x: f32,
    y: f32,
    z: f32,
    w: f32,
}

impl MtVector4 {
    fn to_glam_vec4(&self) -> glam::Vec4 {
        glam::Vec4::new(self.x, self.y, self.z, self.w)
    }
}

#[repr(C, packed)]
#[derive(FromBytes, FromZeroes, Debug, Copy, Clone)]
struct MtAABB {
    minpos: MtVector3,
    maxpos: MtVector3,
}

#[repr(C, packed)]
#[derive(FromBytes, FromZeroes, Debug, Copy, Clone)]

struct MtFloat3A {
    x: f32,
    y: f32,
    z: f32,
}

#[repr(C, packed)]
#[derive(FromBytes, FromZeroes, Debug, Copy, Clone)]
struct MtSphere {
    pos: MtFloat3A,
    r: f32,
}

#[repr(C, packed)]
#[derive(FromBytes, FromZeroes, Copy, Clone)]
struct MtMatrix {
    m: [MtVector4; 4],
}

impl std::fmt::Debug for MtMatrix {
    #[rustfmt::skip]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "
\t{:<8.2} {:<8.2} {:<8.2} {:<8.2}
\t{:<8.2} {:<8.2} {:<8.2} {:<8.2}
\t{:<8.2} {:<8.2} {:<8.2} {:<8.2}
\t{:<8.2} {:<8.2} {:<8.2} {:<8.2}",
            { self.m[0].x },  {self.m[0].y} , {self.m[0].z}, {self.m[0].w},
            { self.m[1].x },  {self.m[1].y} , {self.m[1].z}, {self.m[1].w},
            { self.m[2].x },  {self.m[2].y} , {self.m[2].z}, {self.m[2].w},
            { self.m[3].x },  {self.m[3].y} , {self.m[3].z}, {self.m[3].w},
        )
    }
}

#[repr(C, packed)]
#[derive(FromBytes, FromZeroes, Debug, Copy, Clone)]
struct MtOBB {
    coord: MtMatrix,
    extent: MtVector3,
}

#[repr(C, packed)]
#[derive(FromBytes, FromZeroes, Debug, Copy, Clone)]
struct ModelInfo {
    middist: i32,
    lowdist: i32,
    light_group: u32,
    memory: u16,
    reserved: u16,
}

#[repr(C, packed)]
#[derive(FromBytes, FromZeroes, Debug, Copy, Clone)]
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
    modelinfo: ModelInfo,
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
#[derive(FromBytes, FromZeroes, Debug, Clone)]
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
    boundary: u64, // struct BOUNDARY_INFO *, junk data?
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

    pub fn weight_num(&self) -> u32 {
        (self.very_large_bitfield >> 3) & 0x1f
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

    pub fn vertex_num(&self) -> u32 {
        (self.drawmode_vertexnum >> 16) & 0xffff
    }

    pub fn boundary_num(&self) -> u32 {
        (self.envelope_boundary_connect >> 8) & 0xff
    }
}

#[repr(C, packed)]
#[derive(FromBytes, FromZeroes, Debug, Copy, Clone)]
pub struct PartsInfo {
    no: u32,
    reserved: [u32; 3],
    boundary: MtSphere,
}

#[repr(C, packed)]
#[derive(FromBytes, FromZeroes, Debug, Copy, Clone)]
pub struct BoundaryInfo {
    joint: u32,
    reserved: [u32; 3],
    sphere: MtSphere,
    aabb: MtAABB,
    obb: MtOBB,
}

impl BoundaryInfo {
    pub fn joint(&self) -> u32 {
        self.joint
    }
}

#[repr(C, packed)]
#[derive(FromBytes, FromZeroes, Debug, Copy, Clone)]
pub struct JointInfo {
    bitfield_0x0: u32,
    radius: f32,
    length: f32,
    offset: MtFloat3A,
}
impl JointInfo {
    fn no(&self) -> u32 {
        self.bitfield_0x0 & 0xff
    }

    fn parent(&self) -> u32 {
        (self.bitfield_0x0 >> 8) & 0xff
    }

    fn symmetry(&self) -> u32 {
        (self.bitfield_0x0 >> 16) & 0xff
    }
}

pub struct ModelFile {
    material_names: Vec<String>,
    primitives: Vec<PrimitiveInfo>,
    parts: Vec<PartsInfo>,

    vertex_buf: Vec<u8>,
    index_buf: Vec<u16>,
    boundary_infos: Vec<BoundaryInfo>,
}

impl ModelFile {
    pub fn new<R: Read + Seek>(reader: &mut R) -> anyhow::Result<ModelFile> {
        let header: ModelHdr = util::read_struct(reader)?;

        let boundary_num = util::read_struct::<u32, _>(reader)?;

        debug!("model header: {:#?}", header);
        debug!("boundary_num: {}", boundary_num);

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

        let mut primitive_arr_bytes =
            vec![0u8; header.primitive_num as usize * size_of::<PrimitiveInfo>()];
        reader.seek(std::io::SeekFrom::Start(header.primitive_info as u64))?;
        reader.read_exact(&mut primitive_arr_bytes)?;
        let primitives: Vec<PrimitiveInfo> = util::read_struct_array::<PrimitiveInfo>(&primitive_arr_bytes, header.primitive_num.into())?
            .enumerate()
            .map(|(primitive_idx, primitive)| {
                let primitive = primitive.ok_or_else(|| anyhow!("couldn't read primitive {}", primitive_idx))?;

                debug!(
                    "primitive {}: stride {} (mat {}: {}) layout {:08x} part {} material {} weight_num {} boundary {}",
                    primitive_idx,
                    primitive.vertex_stride(),
                    primitive.material_no() as usize,
                    &material_names[primitive.material_no() as usize],
                    (primitive.inputlayout() & 0xfffff000) >> 0xc,
                    primitive.parts_no(),
                    primitive.material_no(),
                    primitive.weight_num(),
                    primitive.boundary_num(),
                );

                Ok(primitive.clone())
            })
            .collect::<anyhow::Result<_>>()?;

        let mut boundary_info_bytes = vec![0u8; boundary_num as usize * size_of::<BoundaryInfo>()];
        reader.read_exact(&mut boundary_info_bytes)?;
        let boundary_infos: Vec<BoundaryInfo> =
            util::read_struct_array::<BoundaryInfo>(&boundary_info_bytes, boundary_num as usize)?
                .enumerate()
                .map(|(boundary_idx, info)| {
                    let info =
                        info.ok_or_else(|| anyhow!("couldn't read boundary {}", boundary_idx))?;

                    trace!("boundary {}: {:?}", boundary_idx, { info.joint });
                    Ok(*info)
                })
                .collect::<anyhow::Result<_>>()?;

        reader.seek(std::io::SeekFrom::Start(header.joint_info as u64))?;
        if header.jnt_num != 0 {
            let mut joint_info_bytes = vec![0u8; header.jnt_num as usize * size_of::<JointInfo>()];
            reader.read_exact(&mut joint_info_bytes)?;
            let joint_infos: Vec<JointInfo> =
                util::read_struct_array::<JointInfo>(&joint_info_bytes, header.jnt_num.into())?
                    .map(|joint_info| {
                        let joint_info = joint_info.expect("couldn't read joint info");

                        debug!(
                            "joint info: no {} parent {} symmetry {} {:?}",
                            joint_info.no(),
                            joint_info.parent(),
                            joint_info.symmetry(),
                            joint_info,
                        );
                        joint_info.clone()
                    })
                    .collect();

            let lmats =
                util::read_struct_array_stream::<MtMatrix, _>(reader, header.jnt_num.into())?;
            let imats =
                util::read_struct_array_stream::<MtMatrix, _>(reader, header.jnt_num.into())?;

            for lmat in lmats {
                debug!("lmat {:#?}", lmat);
            }

            for imat in imats {
                debug!("imat {:#?}", imat);
            }

            let mut joint_table = [0u8; 0x100];
            reader.read_exact(&mut joint_table)?;
            debug!("joint table {:?}", joint_table);
        }

        let mut parts_arr_bytes = vec![0u8; header.parts_num as usize * size_of::<PartsInfo>()];
        reader.seek(std::io::SeekFrom::Start(header.parts_info as u64))?;
        reader.read_exact(&mut parts_arr_bytes)?;
        let parts: Vec<PartsInfo> =
            util::read_struct_array(&parts_arr_bytes, header.parts_num as usize)?
                .enumerate()
                .map(|(idx, part)| {
                    let part = part.ok_or_else(|| anyhow!("couldn't read part {}", idx))?;
                    debug!("part: {:?}", part);

                    Ok(*part)
                })
                .collect::<anyhow::Result<_>>()?;

        let mut vertex_buf = vec![0u8; header.vertexbuf_size as usize];
        reader.seek(std::io::SeekFrom::Start(header.vertex_data))?;
        reader.read_exact(&mut vertex_buf)?;

        let mut index_buf = vec![0u16; header.index_num as usize];
        reader.seek(std::io::SeekFrom::Start(header.index_data))?;
        reader.read_exact(index_buf.as_mut_slice().as_bytes_mut())?;

        Ok(Self {
            material_names,
            primitives,
            parts,
            boundary_infos,
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

    pub fn parts(&self) -> &[PartsInfo] {
        &self.parts
    }

    pub fn material_names(&self) -> &[String] {
        &self.material_names
    }

    pub fn boundary_infos(&self) -> &[BoundaryInfo] {
        &self.boundary_infos
    }
}

#[test]
fn test_struct_sizes() {
    assert_eq!(size_of::<ModelHdr>(), 0xa0);
    assert_eq!(size_of::<PrimitiveInfo>(), 0x38);
    assert_eq!(size_of::<PartsInfo>(), 0x20);
    assert_eq!(size_of::<BoundaryInfo>(), 0x90);
    assert_eq!(size_of::<JointInfo>(), 24);
    assert_eq!(size_of::<MtMatrix>(), 1 << 6);
}
