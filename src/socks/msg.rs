use std::{io, mem};

use async_std::net::TcpStream;
use ruisutil::bytes::{self, ByteBoxBuf};

//----------------------------------bean
#[repr(C, packed)]
struct MsgInfo {
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

// pub const MaxOther: u64 = 1024 * 1024 * 20; //20M
pub const MAX_HEADS: u64 = 1024 * 1024 * 100; //100M
pub const MAX_BODYS: u64 = 1024 * 1024 * 1024; //1G

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

pub async fn parse_msg(ctxs: &ruisutil::Context, conn: &mut TcpStream) -> io::Result<Message> {
    let bts = ruisutil::tcp_read_async(ctxs, conn, 1).await?;
    if bts.len() < 1 || bts[0] != 0x8du8 {
        return Err(ruisutil::ioerr(
            format!("first byte err:{:?}", &bts[..]),
            None,
        ));
    }
    let bts = ruisutil::tcp_read_async(ctxs, conn, 1).await?;
    if bts.len() < 1 || bts[0] != 0x8fu8 {
        return Err(ruisutil::ioerr(
            format!("second byte err:{:?}", &bts[..]),
            None,
        ));
    }

    let mut info = MsgInfo::new();
    let infoln = mem::size_of::<MsgInfo>();
    let bts = ruisutil::tcp_read_async(ctxs, conn, infoln).await?;
    ruisutil::byte2struct(&mut info, &bts[..])?;
    if (info.len_head) as u64 > MAX_HEADS {
        return Err(ruisutil::ioerr("bytes2 out limit!!", None));
    }
    if (info.len_body) as u64 > MAX_BODYS {
        return Err(ruisutil::ioerr("bytes3 out limit!!", None));
    }

    let mut rt = Message::new();
    rt.version = info.version;
    rt.control = info.control;
    let lnsz = info.len_cmd as usize;
    if lnsz > 0 {
        let bts = ruisutil::tcp_read_async(&ctxs, conn, lnsz).await?;
        rt.cmds = match std::str::from_utf8(&bts[..]) {
            Err(e) => return Err(ruisutil::ioerr("cmd err", None)),
            Ok(v) => String::from(v),
        };
    }
    let lnsz = info.len_head as usize;
    if lnsz > 0 {
        let bts = ruisutil::tcp_read_async(&ctxs, conn, lnsz as usize).await?;
        rt.heads = Some(bts);
    }
    let lnsz = info.len_body as usize;
    if lnsz > 0 {
        let bts = ruisutil::tcp_read_async(&ctxs, conn, lnsz as usize).await?;
        rt.bodys = Some(bts);
    }
    let bts = ruisutil::tcp_read_async(ctxs, conn, 2).await?;
    if bts.len() < 2 || bts[0] != 0x8eu8 || bts[1] != 0x8fu8 {
        return Err(ruisutil::ioerr(
            format!("end byte err:{:?}", &bts[..]),
            None,
        ));
    }

    Ok(rt)
}

pub async fn send_msg(
    ctxs: &ruisutil::Context,
    conn: &mut TcpStream,
    ctrl: i32,
    cmds: Option<String>,
    hds: Option<Box<[u8]>>,
    bds: Option<&[u8]>,
) -> io::Result<()> {
    let mut info = MsgInfo::new();
    // let infoln = mem::size_of::<MsgInfo>();
    info.version = 1;
    info.control = ctrl;
    if let Some(v) = &cmds {
        info.len_cmd = v.len() as u16;
    }
    if let Some(v) = &hds {
        info.len_head = v.len() as u32;
    }
    if let Some(v) = &bds {
        info.len_body = v.len() as u32;
    }
    ruisutil::tcp_write_async(ctxs, conn, &[0x8du8, 0x8fu8]).await?;
    let bts = ruisutil::struct2byte(&info);
    ruisutil::tcp_write_async(ctxs, conn, bts).await?;
    if let Some(v) = &cmds {
        ruisutil::tcp_write_async(ctxs, conn, v.as_bytes()).await?;
    }
    if let Some(v) = &hds {
        ruisutil::tcp_write_async(ctxs, conn, &v[..]).await?;
    }
    if let Some(v) = bds {
        ruisutil::tcp_write_async(ctxs, conn, &v[..]).await?;
    }
    ruisutil::tcp_write_async(ctxs, conn, &[0x8eu8, 0x8fu8]).await?;
    Ok(())
}

pub async fn send_msgs(
    ctxs: &ruisutil::Context,
    conn: &mut TcpStream,
    msg: Messages,
) -> io::Result<()> {
    if let Some(buf) = &msg.bodybuf {
        send_msg_buf(ctxs, conn, msg.control, msg.cmds, msg.heads, Some(buf)).await
    } else if let Some(bds) = &msg.bodys {
        send_msg(ctxs, conn, msg.control, msg.cmds, msg.heads, Some(&bds[..])).await
    } else {
        send_msg(ctxs, conn, msg.control, msg.cmds, msg.heads, None).await
    }
}
pub async fn send_msg_buf(
    ctxs: &ruisutil::Context,
    conn: &mut TcpStream,
    ctrl: i32,
    cmds: Option<String>,
    hds: Option<Box<[u8]>>,
    bds: Option<&ByteBoxBuf>,
) -> io::Result<()> {
    let mut info = MsgInfo::new();
    // let infoln = mem::size_of::<MsgInfo>();
    info.version = 1;
    info.control = ctrl;
    if let Some(v) = &cmds {
        info.len_cmd = v.len() as u16;
    }
    if let Some(v) = &hds {
        info.len_head = v.len() as u32;
    }
    if let Some(v) = bds {
        info.len_body = v.len() as u32;
    }
    ruisutil::tcp_write_async(ctxs, conn, &[0x8du8, 0x8fu8]).await?;
    let bts = ruisutil::struct2byte(&info);
    ruisutil::tcp_write_async(ctxs, conn, bts).await?;
    if let Some(v) = &cmds {
        ruisutil::tcp_write_async(ctxs, conn, v.as_bytes()).await?;
    }
    if let Some(v) = &hds {
        ruisutil::tcp_write_async(ctxs, conn, &v[..]).await?;
    }
    if let Some(v) = bds {
        bytes::tcp_write_async(ctxs, conn, v).await?;
    }
    ruisutil::tcp_write_async(ctxs, conn, &[0x8eu8, 0x8fu8]).await?;
    Ok(())
}



/* pub fn make_udp_packet_v1()->bytes::ByteBox{
  // let rt=bytes::ByteBox::
} */