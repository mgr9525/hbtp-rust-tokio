use std::{collections::HashMap, io, net::SocketAddr, sync::Arc, time::Duration};

use async_std::{channel, net::UdpSocket, sync::RwLock};
use futures::future::BoxFuture;
use ruisutil::bytes;

use super::{msg, udps::UdpMsgParse};

#[derive(Clone)]
pub struct UMsgerServ {
    inner: ruisutil::ArcMut<Inner>,
}
struct Inner {
    ctx: ruisutil::Context,
    addrs: String,
    keys: Option<String>,
    conn: Option<UdpSocket>,
    shuted: bool,

    mutis: RwLock<HashMap<SocketAddr, UdpMsgParse>>,

    //check
    ctms: ruisutil::Timer,
    ctmout: ruisutil::Timer,
    msgs_sx: channel::Sender<msg::Messages>,
    msgs_rx: channel::Receiver<msg::Messages>,

    recver: Box<dyn IUMsgerServ + Send + Sync>,
}

impl UMsgerServ {
    pub fn new<T>(
        ctx: &ruisutil::Context,
        addrs: String,
        keys: Option<String>,
        recver: T,
        sndbufln: usize,
    ) -> Self
    where
        T: IUMsgerServ + Send + Sync + 'static,
    {
        let (sx, rx) = if sndbufln > 0 {
            channel::bounded::<msg::Messages>(sndbufln)
        } else {
            channel::unbounded::<msg::Messages>()
        };
        Self {
            inner: ruisutil::ArcMut::new(Inner {
                ctx: ruisutil::Context::background(Some(ctx.clone())),
                addrs: addrs,
                keys: keys,
                conn: None,
                shuted: false,

                mutis: RwLock::new(HashMap::new()),

                ctms: ruisutil::Timer::new(Duration::from_secs(20)),
                ctmout: ruisutil::Timer::new(Duration::from_secs(30)),
                msgs_sx: sx.clone(),
                msgs_rx: rx,
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
            self.inner.msgs_sx.close();
        }
    }
    pub async fn run(&self) -> async_std::io::Result<()> {
        let ins = unsafe { self.inner.muts() };
        ins.conn = Some(UdpSocket::bind(self.inner.addrs.as_str()).await?);
        self.inner.ctmout.reset();
        let c = self.clone();
        async_std::task::spawn(async move {
            c.run_recv().await;
            println!("UMsgerServ run_recv end!!");
        });
        println!("UMsgerServ start run check");
        while !self.inner.ctx.done() {
            self.run_check().await;
            async_std::task::sleep(Duration::from_millis(100)).await;
        }
        self.stop();
        println!("UMsgerServ end run check");
        Ok(())
    }
    async fn run_recv(&self) {
        let ins = unsafe { self.inner.muts() };
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
                                    print!("run_parse from {} err:{}", src.to_string(), e);
                                }
                            });
                        } else {
                            self.inner.recver.packet_err(src).await;
                        }
                    }
                }
            }
        }
    }
    async fn run_parse(&self, mut buf: bytes::ByteBox, src: SocketAddr) -> io::Result<()> {
        /* if buf.len() < 10 {
            return Err(ruisutil::ioerr(format!("packet len err:{}",buf.len()), None));
        } */
        let bts = buf.cuts(3)?;
        if bts[0] != 0x48 || bts[1] != 0x42 {
            return Err(ruisutil::ioerr(
                format!("packet start err:[{},{}]", buf[0], buf[1]),
                None,
            ));
        }
        if bts[2] != 1 {
            return Err(ruisutil::ioerr(
                format!("packet version err:[{}]", buf[2]),
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
        }

        Ok(())
    }
    async fn run_check(&self) {
        /* println!(
            "m run_check:ctmout={}ms!!!--------------",
            self.inner.ctmout.tmdur().as_millis()
        ); */
        if self.inner.ctmout.tmout() {
            // let _ = self.stop();
            println!("msger heart timeout!!");
            self.inner.ctx.stop();
            return;
        }
    }

    pub async fn remove(&self, src: &SocketAddr) {
        let mut lkv = self.inner.mutis.write().await;
        if let Some(v) = lkv.get(src) {
            v.stop();
        }
        lkv.remove(src);
    }

    pub async fn send(&self, src: &SocketAddr, buf: bytes::ByteBox) {
        let ins = unsafe { self.inner.muts() };
        if let Some(conn) = &self.inner.conn {
            conn.send_to(&buf[..], src).await;
        }
    }
}

pub trait IUMsgerServ {
    fn packet_err(&self, addrs: SocketAddr) -> BoxFuture<'static, ()>;
    fn on_check(&self) -> BoxFuture<'static, ()>;
    fn on_msg(&self, msg: msg::Message) -> BoxFuture<'static, io::Result<()>>;
    // fn on_msg(self, msg: msg::Message) -> Pin<Box<dyn Future<Output = ()> + Sync + Send+'static>>;
}
