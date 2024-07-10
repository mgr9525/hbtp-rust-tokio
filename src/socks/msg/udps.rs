use std::{io, mem};

use ruisutil::bytes;

use super::{
    entity::{self, MsgInfo},
    Messageu, Messageus,
};

pub fn packet_parse(mut buf: bytes::ByteBox) -> io::Result<entity::UdpPackage> {
    let bts = buf.cuts(3)?;
    if bts[0] != 0x48 || bts[1] != 0x42 {
        return Err(ruisutil::ioerr(
            format!("packet start err:[{},{}]", bts[0], bts[1]),
            None,
        ));
    }
    if bts[2] != 1 {
        return Err(ruisutil::ioerr(
            format!("packet version err:[{}]", bts[2]),
            None,
        ));
    }
    let bts = buf.cuts(2)?;
    let ctrl = ruisutil::byte_2i(&bts[..]) as u16;
    let bts = buf.cuts(2)?;
    let ln = ruisutil::byte_2i(&bts[..]) as u16;
    let mut tk = None;
    if ln > 0 {
        let bts = buf.cuts(ln as usize)?;
        let keys = match std::str::from_utf8(&bts[..]) {
            Err(e) => return Err(ruisutil::ioerr(format!("packet keys err:[{}]", e), None)),
            Ok(v) => v,
        };
        tk = Some(keys.to_string());
    }

    Ok(entity::UdpPackage {
        ctrl: ctrl,
        token: tk,
        data: buf,
    })
}
pub fn packet_fmts(ctrl: u16, tks: &Option<String>) -> io::Result<bytes::ByteBoxBuf> {
    let mut buf = bytes::ByteBoxBuf::new();
    buf.push(vec![0x48, 0x42, 1]);
    buf.push(ruisutil::i2_byte(ctrl as i64, 2));
    if let Some(v) = tks {
        let bts = v.as_bytes().to_vec();
        buf.push(ruisutil::i2_byte(bts.len() as i64, 2));
        buf.push(bts);
    } else {
        buf.push(vec![0, 0]);
    }
    Ok(buf)
}

pub fn msg_parse(mut buf: bytes::ByteBox) -> io::Result<Messageu> {
    let bts = buf.cuts(2)?;
    if bts[0] != 0x8e || bts[1] != 0x8f {
        return Err(ruisutil::ioerr(
            format!("msg start err:[{},{}]", bts[0], bts[1]),
            None,
        ));
    }
    let mut info = MsgInfo::new();
    let infoln = mem::size_of::<MsgInfo>();
    let bts = buf.cuts(infoln)?;
    ruisutil::byte2struct(&mut info, &bts[..])?;
    if (info.len_head) as u64 > super::MAX_HEADS {
        return Err(ruisutil::ioerr("bytes2 out limit!!", None));
    }
    if (info.len_body) as u64 > super::MAX_BODYS {
        return Err(ruisutil::ioerr("bytes3 out limit!!", None));
    }

    let mut rt = Messageu::new();
    rt.version = info.version;
    rt.control = info.control;
    let lnsz = info.len_cmd as usize;
    if lnsz > 0 {
        let bts = buf.cuts(lnsz)?;
        rt.cmds = match std::str::from_utf8(&bts[..]) {
            Err(e) => return Err(ruisutil::ioerr("cmd err", None)),
            Ok(v) => String::from(v),
        };
    }
    let lnsz = info.len_head as usize;
    if lnsz > 0 {
        let bts = buf.cuts(lnsz)?;
        rt.heads = Some(bts);
    }
    let lnsz = info.len_body as usize;
    if lnsz > 0 {
        let bts = buf.cuts(lnsz)?;
        rt.bodys = Some(bts);
    }
    Ok(rt)
}

pub fn msg_fmts(data: Messageus) -> io::Result<bytes::ByteBoxBuf> {
    let mut buf = bytes::ByteBoxBuf::new();
    let mut info = MsgInfo::new();
    info.version = 1;
    info.control = data.control;
    if let Some(v) = &data.cmds {
        info.len_cmd = v.len() as u16;
    }
    if let Some(v) = &data.heads {
        info.len_head = v.len() as u32;
    }
    if let Some(v) = &data.bodys {
        info.len_body = v.len() as u32;
    } else if let Some(v) = &data.bodybuf {
        info.len_body = v.len() as u32;
    }
    buf.push(vec![0x8e, 0x8f]);
    let bts = ruisutil::struct2byte(&info).to_vec();
    buf.push(bts);
    if let Some(v) = &data.cmds {
        buf.push(v.as_bytes().to_vec());
    }
    if let Some(v) = &data.heads {
        buf.push(v.clone());
    }
    if let Some(v) = &data.bodys {
        buf.push(v.clone());
    } else if let Some(v) = &data.bodybuf {
        buf.push_all(v);
    }
    Ok(buf)
}
