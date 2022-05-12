use std::{net::SocketAddr, sync::Arc, time::Duration};

use async_std::net::UdpSocket;
use futures::future::BoxFuture;
use ruisutil::bytes;

#[derive(Clone)]
pub struct UMsgerClient {
    inner: ruisutil::ArcMut<Inner>,
}
struct Inner {
    ctx: ruisutil::Context,
    addrs_re: String,
    addrs_lc: String,
    conn: Option<UdpSocket>,
    shuted: bool,

    recver: Box<dyn IUMsgerCli + Send + Sync>,
}

impl UMsgerClient {
    pub fn new<T>(ctx: &ruisutil::Context, addrs_re: String, addrs_lc: String, recver: T) -> Self
    where
        T: IUMsgerCli + Send + Sync + 'static,
    {
        /* let (sx, rx) = if sndbufln > 0 {
            channel::bounded::<msg::Messages>(sndbufln)
        } else {
            channel::unbounded::<msg::Messages>()
        }; */
        Self {
            inner: ruisutil::ArcMut::new(Inner {
                ctx: ruisutil::Context::background(Some(ctx.clone())),
                addrs_re: addrs_re,
                addrs_lc: addrs_lc,
                conn: None,
                shuted: false,

                recver: Box::new(recver),
            }),
        }
    }

    pub fn stop(&self) {
        if !self.inner.shuted {
            println!("msger conn will stop");
            let ins = unsafe { self.inner.muts() };
            ins.shuted = true;
            ins.conn = None;
            self.inner.ctx.stop();
        }
    }
    pub async fn run(&self) -> async_std::io::Result<()> {
        let ins = unsafe { self.inner.muts() };
        let conn = UdpSocket::bind(self.inner.addrs_lc.as_str()).await?;
        conn.connect(self.inner.addrs_re.as_str()).await?;
        ins.conn = Some(conn);
        self.run_recv().await;
        self.stop();
        println!("UMsgerServ end run check");
        Ok(())
    }
    async fn run_recv(&self) {
        while !self.inner.ctx.done() {
            if let Some(conn) = &self.inner.conn {
                let mut buf = vec![0u8; 1500].into_boxed_slice();
                match conn.recv_from(&mut buf[..]).await {
                    Err(e) => {
                        println!("udp recv err:{}", e);
                        self.stop();
                        async_std::task::sleep(Duration::from_millis(5)).await;
                    }
                    Ok((n, src)) => {
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
}

pub trait IUMsgerCli {
    fn packet_err(&self, addrs: &SocketAddr) -> BoxFuture<'static, ()>;
    fn check_token(&self, addrs: &SocketAddr, tks: &Option<String>) -> BoxFuture<'static, bool>;
    fn on_bts(&self, addrs: &SocketAddr, msg: bytes::ByteBox)
        -> BoxFuture<'static, io::Result<()>>;
    fn on_msg(&self, addrs: &SocketAddr, msg: msg::Messageu) -> BoxFuture<'static, io::Result<()>>;
}
