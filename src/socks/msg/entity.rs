use ruisutil::bytes;

//----------------------------------bean
#[repr(C, packed)]
pub struct MsgInfo {
    pub version: u16,
    pub control: i32,
    pub len_cmd: u16,
    pub len_head: u32,
    pub len_body: u32,
}
impl MsgInfo {
    pub fn new() -> Self {
        Self {
            version: 0,
            control: 0,
            len_cmd: 0,
            len_head: 0,
            len_body: 0,
        }
    }
}



#[derive(Clone)]
pub struct UdpPackage {
    pub ctrl: u16,
    pub token: Option<String>,
    pub data: bytes::ByteBox,
}