use std::{io, mem};

use tokio::net::TcpStream;
use ruisutil::bytes::{self, ByteSteamBuf};

use crate::socks::msg::entity::MsgInfo;

use super::{Message, Messages};

pub async fn parse_msg(ctxs: &ruisutil::Context, conn: &mut TcpStream) -> io::Result<Message> {
    let bts = ruisutil::read_all_async(ctxs, conn, 1).await?;
    if bts.len() < 1 || bts[0] != 0x8du8 {
        return Err(ruisutil::ioerr(
            format!("first byte err:{:?}", &bts[..]),
            None,
        ));
    }
    let bts = ruisutil::read_all_async(ctxs, conn, 1).await?;
    if bts.len() < 1 || bts[0] != 0x8fu8 {
        return Err(ruisutil::ioerr(
            format!("second byte err:{:?}", &bts[..]),
            None,
        ));
    }

    let mut info = MsgInfo::new();
    let infoln = mem::size_of::<MsgInfo>();
    let bts = ruisutil::read_all_async(ctxs, conn, infoln).await?;
    ruisutil::byte2struct(&mut info, &bts[..])?;
    if (info.len_head) as u64 > super::MAX_HEADS {
        return Err(ruisutil::ioerr("bytes2 out limit!!", None));
    }
    if (info.len_body) as u64 > super::MAX_BODYS {
        return Err(ruisutil::ioerr("bytes3 out limit!!", None));
    }

    let mut rt = Message::new();
    rt.version = info.version;
    rt.control = info.control;
    let lnsz = info.len_cmd as usize;
    if lnsz > 0 {
        let bts = ruisutil::read_all_async(&ctxs, conn, lnsz).await?;
        rt.cmds = match std::str::from_utf8(&bts[..]) {
            Err(e) => return Err(ruisutil::ioerr("cmd err", None)),
            Ok(v) => String::from(v),
        };
    }
    let lnsz = info.len_head as usize;
    if lnsz > 0 {
        let bts = ruisutil::read_all_async(&ctxs, conn, lnsz as usize).await?;
        rt.heads = Some(bytes::ByteBox::from(bts));
    }
    let lnsz = info.len_body as usize;
    if lnsz > 0 {
        let bts = ruisutil::read_all_async(&ctxs, conn, lnsz as usize).await?;
        rt.bodys = Some(bytes::ByteBox::from(bts));
    }
    let bts = ruisutil::read_all_async(ctxs, conn, 2).await?;
    if bts.len() < 2 || bts[0] != 0x8eu8 || bts[1] != 0x8fu8 {
        return Err(ruisutil::ioerr(
            format!("end byte err:{:?}", &bts[..]),
            None,
        ));
    }

    Ok(rt)
}
pub async fn parse_steam_msg(buf: &ByteSteamBuf) -> io::Result<Message> {
    let bts = buf.pull_size(None, 1).await?.to_bytes();
    if bts.len() < 1 || bts[0] != 0x8du8 {
        return Err(ruisutil::ioerr(
            format!("first byte err:{:?}", &bts[..]),
            None,
        ));
    }
    let bts = buf.pull_size(None, 1).await?.to_bytes();
    if bts.len() < 1 || bts[0] != 0x8fu8 {
        return Err(ruisutil::ioerr(
            format!("second byte err:{:?}", &bts[..]),
            None,
        ));
    }

    let mut info = MsgInfo::new();
    let infoln = mem::size_of::<MsgInfo>();
    let bts = buf.pull_size(None, infoln).await?.to_bytes();
    ruisutil::byte2struct(&mut info, &bts[..])?;
    if (info.len_head) as u64 > super::MAX_HEADS {
        return Err(ruisutil::ioerr("bytes2 out limit!!", None));
    }
    if (info.len_body) as u64 > super::MAX_BODYS {
        return Err(ruisutil::ioerr("bytes3 out limit!!", None));
    }

    let mut rt = Message::new();
    rt.version = info.version;
    rt.control = info.control;
    let lnsz = info.len_cmd as usize;
    if lnsz > 0 {
        let bts = buf.pull_size(None, lnsz).await?.to_bytes();
        rt.cmds = match std::str::from_utf8(&bts[..]) {
            Err(e) => return Err(ruisutil::ioerr("cmd err", None)),
            Ok(v) => String::from(v),
        };
    }
    let lnsz = info.len_head as usize;
    if lnsz > 0 {
        let bts = buf.pull_size(None, lnsz).await?.to_bytes();
        rt.heads = Some(bytes::ByteBox::from(bts));
    }
    let lnsz = info.len_body as usize;
    if lnsz > 0 {
        let bts = buf.pull_size(None, lnsz).await?;
        // rt.bodys = Some(bts.to_bytes());
        rt.bodybuf = Some(bts);
    }
    let bts = buf.pull_size(None, 2).await?.to_bytes();
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
    hds: &Option<bytes::ByteBox>,
    bds: Option<&[u8]>,
) -> io::Result<()> {
    let mut info = MsgInfo::new();
    // let infoln = mem::size_of::<MsgInfo>();
    info.version = 1;
    info.control = ctrl;
    if let Some(v) = &cmds {
        info.len_cmd = v.len() as u16;
    }
    if let Some(v) = hds {
        info.len_head = v.len() as u32;
    }
    if let Some(v) = &bds {
        info.len_body = v.len() as u32;
    }
    ruisutil::write_all_async(ctxs, conn, &[0x8du8, 0x8fu8]).await?;
    let bts = ruisutil::struct2byte(&info);
    ruisutil::write_all_async(ctxs, conn, bts).await?;
    if let Some(v) = &cmds {
        ruisutil::write_all_async(ctxs, conn, v.as_bytes()).await?;
    }
    if let Some(v) = hds {
        ruisutil::write_all_async(ctxs, conn, &v[..]).await?;
    }
    if let Some(v) = bds {
        ruisutil::write_all_async(ctxs, conn, &v[..]).await?;
    }
    ruisutil::write_all_async(ctxs, conn, &[0x8eu8, 0x8fu8]).await?;
    Ok(())
}

pub async fn send_msgs(
    ctxs: &ruisutil::Context,
    conn: &mut TcpStream,
    msg: Messages,
) -> io::Result<()> {
    if let Some(buf) = &msg.bodybuf {
        send_msg_buf(ctxs, conn, msg.control, msg.cmds, &msg.heads, Some(buf)).await
    } else if let Some(bds) = &msg.bodys {
        send_msg(ctxs, conn, msg.control, msg.cmds, &msg.heads, Some(&bds[..])).await
    } else {
        send_msg(ctxs, conn, msg.control, msg.cmds, &msg.heads, None).await
    }
}
pub async fn send_msg_buf(
    ctxs: &ruisutil::Context,
    conn: &mut TcpStream,
    ctrl: i32,
    cmds: Option<String>,
    hds: &Option<bytes::ByteBox>,
    bds: Option<&bytes::ByteBoxBuf>,
) -> io::Result<()> {
    let mut info = MsgInfo::new();
    // let infoln = mem::size_of::<MsgInfo>();
    info.version = 1;
    info.control = ctrl;
    if let Some(v) = &cmds {
        info.len_cmd = v.len() as u16;
    }
    if let Some(v) = hds {
        info.len_head = v.len() as u32;
    }
    if let Some(v) = bds {
        info.len_body = v.len() as u32;
    }
    ruisutil::write_all_async(ctxs, conn, &[0x8du8, 0x8fu8]).await?;
    let bts = ruisutil::struct2byte(&info);
    ruisutil::write_all_async(ctxs, conn, bts).await?;
    if let Some(v) = &cmds {
        ruisutil::write_all_async(ctxs, conn, v.as_bytes()).await?;
    }
    if let Some(v) = hds {
        ruisutil::write_all_async(ctxs, conn, &v[..]).await?;
    }
    if let Some(v) = bds {
        let bts=v.to_byte_box();
        ruisutil::write_all_async(ctxs, conn, &bts).await?;
    }
    ruisutil::write_all_async(ctxs, conn, &[0x8eu8, 0x8fu8]).await?;
    Ok(())
}
