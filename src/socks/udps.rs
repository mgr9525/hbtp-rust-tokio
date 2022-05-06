use std::{io, net::SocketAddr, time::Duration};

use async_std::sync::{Mutex, RwLock};
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

    msgs: Vec<UdpMsgMerge>,

    started: bool,
    pushed: bool,
    curr_id: u8,
    curr_ln: u16,
    count: u16,

    tmout: ruisutil::Timer,
    ctmout: ruisutil::Timer,
    datas: RwLock<Vec<Option<bytes::ByteBox>>>,
}

impl UdpMsgParse {
    pub fn new(ctx: &ruisutil::Context, prt: UMsgerServ, addrs: SocketAddr) -> Self {
        let ctxs = ruisutil::Context::background(Some(ctx.clone()));
        Self {
            inner: ruisutil::ArcMut::new(Inner {
                prt: prt,
                ctx: ctxs.clone(),
                addrs: addrs,

                msgs: vec![UdpMsgMerge::new(&ctxs); 256],

                started: false,
                pushed: false,
                curr_id: 0,
                curr_ln: 0,
                count: 0,

                tmout: ruisutil::Timer::new(Duration::from_millis(10)),
                ctmout: ruisutil::Timer::new(Duration::from_secs(30)),
                datas: RwLock::new(Vec::new()),
            }),
        }
    }

    pub fn stop(&self) {
        self.inner.ctx.stop();
        let ins = unsafe { self.inner.muts() };
        ins.started = false;
    }
    pub async fn start(&self) {
        let c = self.clone();
        async_std::task::spawn(async move {
            c.run().await;
        });
    }

    async fn run(&self) {
        let ins = unsafe { self.inner.muts() };
        self.inner.ctmout.reset();
        while !self.inner.ctx.done() {
            if self.inner.ctmout.tmout() {
                self.stop();
            }
            if self.inner.started {
                if self.inner.tmout.tmout() {
                    if self.check().await {
                        self.push().await;
                    }
                }
            }
            async_std::task::sleep(Duration::from_millis(5)).await;
        }
        self.inner.prt.remove(&self.inner.addrs).await;
    }

    pub async fn ctrls(&self, buf: bytes::ByteBox, ctrl: u8, id: u8, ind: u16, ln: u16) {
        self.inner.ctmout.reset();
        match ctrl {
            11 => {} //客户端发送了
            20 => self.parse(buf, id, ind, ln).await,
            _ => {}
        }
    }
    pub async fn parse(&self, buf: bytes::ByteBox, id: u8, ind: u16, ln: u16) {
        let ins = unsafe { self.inner.muts() };
        if id != self.inner.curr_id {
            ins.pushed = false;
            ins.curr_id = id;
            ins.curr_ln = ln;
            ins.count = 0;
            {
                let mut lkv = self.inner.datas.write().await;
                *lkv = vec![None; ln as usize];
            }
        }
        if ln != self.inner.curr_ln {
            return;
        }
        if !self.inner.started {
            ins.started = true;
            self.inner.tmout.reset();
        }
        self.inner.ctmout.reset();

        let mut lkv = self.inner.datas.write().await;
        lkv[ind as usize] = Some(buf);
        ins.count += 1;
        if ins.count >= ln {
            self.push().await;
        }
    }

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
    }
    async fn push(&self) {
        if self.inner.pushed {
            return;
        }
        let ins = unsafe { self.inner.muts() };
    }
}

#[derive(Clone)]
struct UdpMsgMerge {
    inner: ruisutil::ArcMut<Inners>,
}

struct Inners {
    ctx: ruisutil::Context,
}
impl UdpMsgMerge {
    pub fn new(ctx: &ruisutil::Context) -> Self {
        Self {
            inner: ruisutil::ArcMut::new(Inners { ctx: ctx.clone() }),
        }
    }
}
