use std::{io, net::SocketAddr, time::Duration};

use tokio::{sync::{Mutex, RwLock}, task};
use ruisutil::bytes;

use super::udp::UMsgerServ;

#[derive(Clone)]
pub struct UdpMsgParse {
    inner: ruisutil::ArcMut<Inner>,
}

struct Inner {
    prt: UMsgerServ,
    ctx: ruisutil::Context,
    addrs: SocketAddr,

    curr: u8,
    msgs: Vec<UdpMsgMerge>,
    ctmout: ruisutil::Timer,
}

impl UdpMsgParse {
    pub fn new(ctx: &ruisutil::Context, prt: UMsgerServ, addrs: SocketAddr) -> Self {
        let ctxs = ruisutil::Context::background(Some(ctx.clone()));
        Self {
            inner: ruisutil::ArcMut::new(Inner {
                prt: prt,
                ctx: ctxs.clone(),
                addrs: addrs,

                curr: 0,
                msgs: vec![UdpMsgMerge::new(&ctxs); 256],
                ctmout: ruisutil::Timer::new(Duration::from_secs(30)),
            }),
        }
    }

    pub fn stop(&self) {
        self.inner.ctx.stop();
    }
    pub async fn start(&self) {
        let c = self.clone();
        task::spawn(async move {
            c.run().await;
        });
    }

    async fn run(&self) {
        let ins = unsafe { self.inner.muts() };
        self.inner.ctmout.reset();
        for it in &self.inner.msgs {
            it.set_prt(self.clone());
        }
        let mut i = 0;
        while !self.inner.ctx.done() {
            if self.inner.ctmout.tmout() {
                self.stop();
            }

            i = self.inner.curr as usize;
            loop {
                i += 1;
                if i >= 255 {
                    i = 1;
                }
                let it = &self.inner.msgs[i];
                if it.is_finished() {
                    break;
                }
                if it.tick() {
                    // send re
                }
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
        self.inner.prt.remove(&self.inner.addrs).await;
    }

    fn canuse_id(&self, id: u8) -> bool {
        if self.inner.curr == 0 {
            return true;
        }
        let tmp = (self.inner.curr as u16) + 100;
        if id >= self.inner.curr {
            if self.inner.curr - id < 100 {
                return true;
            }
        } else if tmp >= 255 {
            if id < (tmp - 254) as u8 {
                return true;
            }
        }

        false
    }
    pub async fn ctrls(&self, buf: bytes::ByteBox, ctrl: u8, id: u8, ind: u16, ln: u16) {
        self.inner.ctmout.reset();
        match ctrl {
            //客户端请求发送
            11 => if self.canuse_id(id) {},
            20 => {
                if self.canuse_id(id) {
                    self.parse(buf, id, ind, ln).await
                }
            }
            _ => {}
        }
    }
    pub async fn parse(&self, buf: bytes::ByteBox, id: u8, ind: u16, ln: u16) {
        if self.inner.curr == 0 {
            unsafe { self.inner.muts().curr = id };
        }
        let it = &self.inner.msgs[id as usize];
        it.push(buf, ind, ln).await;
    }
    /*
    async fn check(&self) -> bool {
        let lkv = self.inner.datas.read().await;
        let itrs = lkv.iter();
        let mut i = 0u32;
        let mut inds = Vec::with_capacity(10);
        for it in itrs {
            if let None = it {
                inds.push(i);
                i += 1;
            }
        }

        // send needs

        inds.len() <= 0
    } */
    async fn push(&self) {
        // let ins = unsafe { self.inner.muts() };
    }
}

#[derive(Clone)]
struct UdpMsgMerge {
    inner: ruisutil::ArcMut<Inners>,
}

struct Inners {
    prt: Option<UdpMsgParse>,
    ctx: ruisutil::Context,
    finished: bool,

    ln: u16,
    count: u16,
    tmout: ruisutil::Timer,
    datas: RwLock<Vec<Option<bytes::ByteBox>>>,
}
impl UdpMsgMerge {
    pub fn new(ctx: &ruisutil::Context) -> Self {
        Self {
            inner: ruisutil::ArcMut::new(Inners {
                prt: None,
                ctx: ctx.clone(),
                finished: true,
                ln: 0,
                count: 0,
                tmout: ruisutil::Timer::new(Duration::from_millis(5)),
                datas: RwLock::new(Vec::new()),
            }),
        }
    }

    pub fn set_prt(&self, a: UdpMsgParse) {
        unsafe { self.inner.muts().prt = Some(a) };
    }

    pub async fn push(&self, buf: bytes::ByteBox, ind: u16, ln: u16) {
        let ins = unsafe { self.inner.muts() };
        if self.inner.finished || self.inner.ln != ln {
            {
                let mut lkv = self.inner.datas.write().await;
                *lkv = vec![None; ln as usize];
            }
            ins.ln = ln;
            ins.count = 0;
            ins.finished = false;
        }
        let mut lkv = self.inner.datas.write().await;
        if let None = &lkv[ind as usize] {
            lkv[ind as usize] = Some(buf);
            ins.count += 1;
        }
        if ins.count >= ln {
            ins.finished = true;
            if let Some(v) = &self.inner.prt {
                v.push().await;
            }
            {
                let mut lkv = self.inner.datas.write().await;
                *lkv = Vec::new();
            }
        }

        self.inner.tmout.reset();
    }

    pub fn is_finished(&self) -> bool {
        self.inner.finished
    }
    pub fn tick(&self) -> bool {
        if self.inner.finished {
            false
        } else {
            self.inner.tmout.tick()
        }
    }
}
