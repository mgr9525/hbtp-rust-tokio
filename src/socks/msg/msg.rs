use ruisutil::bytes;

pub struct Messages {
    pub control: i32,
    pub cmds: Option<String>,
    pub heads: Option<Box<[u8]>>,
    pub bodys: Option<Box<[u8]>>,
    pub bodybuf: Option<bytes::ByteBoxBuf>,
}
pub struct Message {
    pub version: u16,
    pub control: i32,
    pub cmds: String,
    pub heads: Option<Box<[u8]>>,
    pub bodys: Option<Box<[u8]>>,
}
impl Message {
    pub fn new() -> Self {
        Self {
            version: 0,
            control: 0,
            cmds: String::new(),
            heads: None,
            bodys: None,
        }
    }
    pub fn own_bodys(&mut self) -> Option<Box<[u8]>> {
        std::mem::replace(&mut self.bodys, None)
    }
}

pub struct Messageus {
    pub control: i32,
    pub cmds: Option<String>,
    pub heads: Option<bytes::ByteBox>,
    pub bodys: Option<bytes::ByteBox>,
    pub bodybuf: Option<bytes::ByteBoxBuf>,
}
pub struct Messageu {
    pub version: u16,
    pub control: i32,
    pub cmds: String,
    pub heads: Option<bytes::ByteBox>,
    pub bodys: Option<bytes::ByteBox>,
}
impl Messageu {
    pub fn new() -> Self {
        Self {
            version: 0,
            control: 0,
            cmds: String::new(),
            heads: None,
            bodys: None,
        }
    }
}

/* pub fn make_udp_packet_v1()->bytes::ByteBox{
  // let rt=bytes::ByteBox::
} */
