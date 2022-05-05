use std::{io, time::Duration};

use async_std::{channel, net::UdpSocket};
use futures::future::BoxFuture;

use super::msg;

#[derive(Clone)]
pub struct UMsgerServ<T: IUMsgerServ + Clone + Sync + Send + 'static> {
    inner: ruisutil::ArcMut<Inner<T>>,
}
struct Inner<T: IUMsgerServ + Clone> {
    ctx: ruisutil::Context,
    addrs: String,
    conn: Option<UdpSocket>,
    shuted: bool,
    //check
    ctms: ruisutil::Timer,
    ctmout: ruisutil::Timer,
    msgs_sx: channel::Sender<msg::Messages>,
    msgs_rx: channel::Receiver<msg::Messages>,

    recver: T,
}

impl<T: IUMsgerServ + Clone + Sync + Send + 'static> UMsgerServ<T> {
    pub fn new(ctx: &ruisutil::Context, addrs: String, recver: &T, sndbufln: usize) -> Self {
        let (sx, rx) = if sndbufln > 0 {
            channel::bounded::<msg::Messages>(sndbufln)
        } else {
            channel::unbounded::<msg::Messages>()
        };
        Self {
            inner: ruisutil::ArcMut::new(Inner {
                ctx: ruisutil::Context::background(Some(ctx.clone())),
                addrs: addrs,
                conn: None,
                shuted: false,

                ctms: ruisutil::Timer::new(Duration::from_secs(20)),
                ctmout: ruisutil::Timer::new(Duration::from_secs(30)),
                msgs_sx: sx.clone(),
                msgs_rx: rx,
                recver: recver.clone(),
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
        let mut buf = [0u8; 2000];
        while !self.inner.ctx.done() {
            if let Some(conn) = &self.inner.conn {
                match conn.recv_from(&mut buf).await {
                    Err(e) => println!("udp recv err:{}", e),
                    Ok((n, src)) => match src.ip() {
                        std::net::IpAddr::V4(ip) => {
                          //ip.octets()
                        }
                        std::net::IpAddr::V6(v) => {}
                    },
                }
            }
        }
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
}

pub trait IUMsgerServ: Clone {
    fn on_check(self) -> BoxFuture<'static, ()>;
    fn on_msg(self, msg: msg::Message) -> BoxFuture<'static, io::Result<()>>;
    // fn on_msg(self, msg: msg::Message) -> Pin<Box<dyn Future<Output = ()> + Sync + Send+'static>>;
}
