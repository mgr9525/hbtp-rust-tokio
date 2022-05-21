use std::{collections::HashMap, io, net::SocketAddr, sync::Arc, time::Duration};

use async_std::{
    channel,
    net::{ToSocketAddrs, UdpSocket},
    sync::RwLock,
};
use futures::future::BoxFuture;
use ruisutil::bytes;

use crate::socks::msg;

use super::udps::UdpMsgParse;
// #[cfg(unix)]
// use std::os::unix::prelude::*;

#[derive(Clone)]
pub struct UMsgerServ {
    inner: ruisutil::ArcMut<Inner>,
}
struct Inner {
    ctx: ruisutil::Context,
    addrs: String,
    conn: Option<UdpSocket>,
    shuted: bool,

    mutis: RwLock<HashMap<SocketAddr, UdpMsgParse>>,
    recver: Box<dyn IUMsgerServ + Send + Sync>,
}

impl UMsgerServ {
    pub fn new<T>(ctx: &ruisutil::Context, addrs: String, recver: T) -> Self
    where
        T: IUMsgerServ + Send + Sync + 'static,
    {
        /* let (sx, rx) = if sndbufln > 0 {
            channel::bounded::<msg::Messages>(sndbufln)
        } else {
            channel::unbounded::<msg::Messages>()
        }; */
        Self {
            inner: ruisutil::ArcMut::new(Inner {
                ctx: ruisutil::Context::background(Some(ctx.clone())),
                addrs: addrs,
                conn: None,
                shuted: false,

                mutis: RwLock::new(HashMap::new()),
                recver: Box::new(recver),
            }),
        }
    }

    pub fn stop(&self) {
        if !self.inner.shuted {
            println!("udp_msger conn will stop");
            let ins = unsafe { self.inner.muts() };
            ins.shuted = true;
            self.inner.ctx.stop();
            // c.shutdown();
            /* let conn = std::mem::replace(&mut ins.conn, None);
            if let Some(v) = conn {
                std::mem::drop(v);
            } */
            ins.conn = None;
        }
    }

    /* #[cfg(unix)]
    fn shutdown(&self) {
        extern "C" {
            fn shutdown(socket: i32, how: i32) -> i32;
        }
        if let Some(conn)=&self.inner.conn {
            // libc::shubd
            let fd = conn.as_raw_fd();
            let frt=unsafe { shutdown(fd, 2) };
            println!("udp {} shutdown frt:{}",fd,frt);
        }
    }
    #[cfg(not(unix))]
    fn shutdown(&self) {} */

    /* pub async fn remote_addr(&self) -> Option<SocketAddr> {
        match &self.inner.conn {
            None => None,
            Some(v) => match v.peer_addr() {
                Err(_) => None,
                Ok(vs) => Some(vs),
            },
        }
    } */
    pub async fn connect<T: ToSocketAddrs>(&self, addr: T) -> io::Result<()> {
        if let Some(v) = &self.inner.conn {
            v.connect(addr).await?;
        }
        Ok(())
    }
    pub async fn run(&self) -> io::Result<()> {
        let ins = unsafe { self.inner.muts() };
        let conn = UdpSocket::bind(self.inner.addrs.as_str()).await?;
        ins.conn = Some(conn);
        self.run_recv().await;
        self.stop();
        println!("udp_msger end run check");
        Ok(())
    }
    async fn run_recv(&self) {
        while !self.inner.ctx.done() {
            if let Some(conn) = &self.inner.conn {
                let mut buf = vec![0u8; 1500].into_boxed_slice();
                match conn.recv_from(&mut buf[..]).await {
                    Err(e) => {
                        println!("udp_msger recv err:{}", e);
                        self.stop();
                        async_std::task::sleep(Duration::from_millis(5)).await;
                    }
                    Ok((n, src)) => {
                        // println!("udp_msger recv_from({}):{}", src.to_string().as_str(), n);
                        if n <= 1472 {
                            let c = self.clone();
                            async_std::task::spawn(async move {
                                let bts = bytes::ByteBox::new(Arc::new(buf), 0, n);
                                if let Err(e) = c.run_parse(bts, src.clone()).await {
                                    println!("run_parse from {} err:{}", src.to_string(), e);
                                }
                            });
                        } else {
                            self.inner.recver.packet_err(&src).await;
                        }
                    }
                }
            }
        }
    }
    async fn run_parse(&self, buf: bytes::ByteBox, src: SocketAddr) -> io::Result<()> {
        /* if buf.len() < 10 {
            return Err(ruisutil::ioerr(format!("packet len err:{}",buf.len()), None));
        } */
        let pckt = msg::udps::packet_parse(buf)?;
        if !self.inner.recver.check_token(&src, &pckt.token).await {
            return Err(ruisutil::ioerr("packet token err!!!", None));
        }

        /* println!("parse packet ctrl:{}", pckt.ctrl);
        print!("datas:");
        ruisutil::print_hex(&pckt.data[..]);
        println!(";"); */
        /*
          10-20: 无需按顺序(包小于1400)
          20-30: 需要重组,无需按顺序(包可大于1400)
          30-40: 需要重组,需要按顺序(包可大于1400)
        */
        match pckt.ctrl {
            11 => {
                self.inner.recver.on_bts(&src, pckt.data.clone()).await?;
            }
            12 => {
                let m = msg::udps::msg_parse(pckt.data.clone())?;
                self.inner.recver.on_msg(&src, m).await?;
            }
            _ => {}
        }
        /*
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
        let ctrl = bts[0];
        let id = bts[1];
        let bts = buf.cuts(2)?;
        let ind = ruisutil::byte_2i(&bts[..]) as u16;
        let bts = buf.cuts(2)?;
        let ln = ruisutil::byte_2i(&bts[..]) as u16;
        let bts = buf.cuts(2)?;
        let kn = ruisutil::byte_2i(&bts[..]) as usize;
        let bts = buf.cuts(kn)?;
        if let Some(vs) = &self.inner.keys {
            if !vs.is_empty() {
                let keys = match std::str::from_utf8(&bts[..]) {
                    Err(e) => {
                        return Err(ruisutil::ioerr(format!("packet keys err:[{}]", e), None))
                    }
                    Ok(v) => v,
                };
                if !vs.eq(keys) {
                    return Err(ruisutil::ioerr(format!("packet keys err:{}", keys), None));
                }
            }
        }
        if id == 0 || ln <= 0 || ind >= ln {
            return Err(ruisutil::ioerr(
                format!("packet param err:id={},ind={},ln={}", id, ind, ln),
                None,
            ));
        }
        let mut pn = None;
        {
            let lkv = self.inner.mutis.read().await;
            if let Some(v) = lkv.get(&src) {
                pn = Some(v.clone());
            }
        }
        match pn {
            Some(v) => {
                v.ctrls(buf, ctrl, id, ind, ln).await;
            }
            None => {
                let mut lkv = self.inner.mutis.write().await;
                let v = UdpMsgParse::new(&self.inner.ctx, self.clone(), src.clone());
                v.start().await;
                v.ctrls(buf, ctrl, id, ind, ln).await;
                lkv.insert(src, v);
            }
        } */

        Ok(())
    }

    pub async fn remove(&self, src: &SocketAddr) {
        let mut lkv = self.inner.mutis.write().await;
        if let Some(v) = lkv.get(src) {
            v.stop();
        }
        lkv.remove(src);
    }

    pub async fn send1bts(
        &self,
        data: bytes::ByteBox,
        tks: Option<String>,
        dist: Option<&SocketAddr>,
    ) -> io::Result<()> {
        if data.len() > 1200 {
            return Err(ruisutil::ioerr("data len out packet", None));
        }
        let mut bts = msg::udps::packet_fmts(11, tks)?;
        bts.push(data);
        if let Some(conn) = &self.inner.conn {
            if let Some(addr) = dist {
                conn.send_to(&bts.to_bytes()[..], addr).await?;
            } else {
                conn.send(&bts.to_bytes()[..]).await?;
            }
        }
        Ok(())
    }
    pub async fn send1msg(
        &self,
        data: msg::Messageus,
        tks: Option<String>,
        dist: Option<&SocketAddr>,
    ) -> io::Result<()> {
        let datas = msg::udps::msg_fmts(data)?;
        if datas.len() > 1200 {
            return Err(ruisutil::ioerr("msg len out packet", None));
        }
        let mut bts = msg::udps::packet_fmts(12, tks)?;
        bts.push_all(&datas);
        if let Some(conn) = &self.inner.conn {
            if let Some(addr) = dist {
                conn.send_to(&bts.to_bytes()[..], addr).await?;
            } else {
                conn.send(&bts.to_bytes()[..]).await?;
            }
        }
        Ok(())
    }
}

pub trait IUMsgerServ {
    fn packet_err(&self, addrs: &SocketAddr) -> BoxFuture<'static, ()>;
    fn check_token(&self, addrs: &SocketAddr, tks: &Option<String>) -> BoxFuture<'static, bool>;
    fn on_bts(&self, addrs: &SocketAddr, msg: bytes::ByteBox)
        -> BoxFuture<'static, io::Result<()>>;
    fn on_msg(&self, addrs: &SocketAddr, msg: msg::Messageu) -> BoxFuture<'static, io::Result<()>>;
}
