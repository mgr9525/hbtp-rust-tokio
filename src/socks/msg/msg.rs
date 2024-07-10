use std::sync::Arc;

use ruisutil::bytes;

#[derive(Clone)]
pub struct Messages {
    pub control: i32,
    pub cmds: Option<String>,
    pub heads: Option<bytes::ByteBox>,
    pub bodys: Option<bytes::ByteBox>,
    pub bodybuf: Option<Arc<bytes::ByteBoxBuf>>,
}
#[derive(Clone)]
pub struct Message {
    pub version: u16,
    pub control: i32,
    pub cmds: String,
    pub heads: Option<bytes::ByteBox>,
    pub bodys: Option<bytes::ByteBox>,
    pub bodybuf: Option<bytes::ByteBoxBuf>,
}
impl Message {
    pub fn new() -> Self {
        Self {
            version: 0,
            control: 0,
            cmds: String::new(),
            heads: None,
            bodys: None,
            bodybuf: None,
        }
    }
    pub fn own_bodys(&mut self) -> Option<bytes::ByteBox> {
        std::mem::replace(&mut self.bodys, None)
    }
    pub fn own_bodybuf(&mut self) -> Option<bytes::ByteBoxBuf> {
        std::mem::replace(&mut self.bodybuf, None)
    }
    pub fn body_box(&self) -> Option<bytes::ByteBox> {
        if let Some(v) = &self.bodys {
            Some(v.clone())
        } else if let Some(v) = &self.bodybuf {
            Some(v.to_byte_box())
        } else {
            None
        }
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
