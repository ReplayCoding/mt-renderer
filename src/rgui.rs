use std::io::Read;

struct GuiFileData {}

struct GuiFile {}

impl GuiFile {
    pub fn new<R: Read>(reader: &mut R) -> anyhow::Result<Self> {
        todo!()
    }
}
