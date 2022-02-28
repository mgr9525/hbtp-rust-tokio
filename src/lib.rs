// extern crate proc_macro;
extern crate async_std;
extern crate futures;
extern crate qstring;
extern crate ruisutil;
extern crate serde;
extern crate serde_json;

use std::{
    collections::{HashMap, LinkedList},
    io,
    time::Duration,
};

use async_std::{
    net::{TcpListener, TcpStream},
    task,
};
use async_std::{prelude::*, sync::RwLock};
use futures::future::{BoxFuture, Future};

pub use req::Request;
pub use req::Response;
pub use res::Context;
pub use res::{LmtMaxConfig, LmtTmConfig};
// pub use res::CtxInner;

mod req;
mod res;

#[cfg(test)]
mod tests {
    use std::{thread, time::Duration};

    use async_std::task;

    use qstring::QString;

    use crate::{Engine, Request};

    /* #[test]
    fn it_works() {
        println!("hello test");
        let ctx1 = ruisutil::Context::background(None);
        let ctx2 = ruisutil::Context::background(Some(ctx1.clone()));
        println!("start:ctx1:{},ctx2:{}", ctx1.done(), ctx2.done());
        ctx2.stop();
        println!("end:ctx1:{},ctx2:{}", ctx1.done(), ctx2.done());

        let wg = ruisutil::WaitGroup::new();
        let wg1 = wg.clone();
        thread::spawn(move || {
            let mut info = MsgInfo::new();
            info.version = 1;
            info.control = 2;
            info.lenCmd = 1000;
            let bts = ruisutil::struct2byte(&info);
            let ln = std::mem::size_of::<MsgInfo>();
            println!(
                "MsgInfo info.v:{},bts({}/{}):{:?}",
                info.version,
                bts.len(),
                ln,
                bts
            );
            let mut infos = MsgInfo::new();
            if let Ok(()) = ruisutil::byte2struct(&mut infos, bts) {
                println!(
                    "MsgInfos infos.v:{},ctrl:{},cmdln:{}",
                    infos.version, infos.control, infos.lenCmd
                );
            }
            thread::sleep_ms(3000);
            println!("thread end!!!!!");
            drop(wg1);
        });
        println!("start wg.wait");
        wg.wait();
        println!("start wg.wait end!!!!!");
        thread::sleep_ms(500);
    } */

    #[test]
    fn hbtp_server() {
        let mut serv = Engine::new(None, "0.0.0.0:7030");
        println!("hbtp serv start!!!");
        // let cb = move |ctx: &mut crate::Context| testFun(ctx);
        // let fun = Box::new(cb);
        // let func = |ctx| Box::pin(testFun(ctx));
        serv.reg_fun(1, testFun, None);
        task::block_on(Engine::run(serv));
    }
    async fn testFun(c: crate::Context) -> std::io::Result<()> {
        println!(
            "testFun ctrl:{},cmd:{},ishell:{},arg hello1:{}",
            c.control(),
            c.command(),
            c.command() == "hello",
            c.get_arg("hehe1").unwrap().as_str()
        );
        /* if let Some(v) = c.get() {
            let cs = v.read().await;
            cs.get_bodys();
        } */
        // panic!("whats?");
        c.res_string(crate::ResCodeOk, "hello,there is rust!!")
            .await?;
        Ok(())
    }
    #[test]
    fn hbtp_request() {
        async_std::task::block_on(async {
            let mut req = Request::new("localhost:7030", 1);
            req.command("hello");
            req.add_arg("hehe1", "123456789");
            match req.do_string(None, "dedededede").await {
                Err(e) => println!("do err:{}", e),
                Ok(res) => {
                    println!("res code:{}", res.get_code());
                    if let Some(bs) = res.get_bodys() {
                        println!("res data:{}", std::str::from_utf8(&bs[..]).unwrap())
                    }
                }
            };
        });
    }
    #[test]
    fn qstring_test() {
        let mut qs = QString::from("foo=bar");
        qs.add_pair(("haha", "hehe"));
        let val = qs.get("foo").unwrap();
        println!("val:{},s:{}", val, qs.to_string());
    }
    #[test]
    fn ctx_test() {
        let ctx = ruisutil::Context::with_timeout(None, Duration::from_secs(5));
        while !ctx.done() {
            println!("running");
            thread::sleep(Duration::from_millis(500));
        }
        println!("end!!!");
    }
}
// type ConnFun = fn(res: &mut Context);
// type ConnFun = impl Fn(i32) -> Future;
// type ConnFun = fn(res: &mut Context)->Pin<Box<dyn Future<Output = ()>>>;
// pub type ConnFuture = Box<dyn Future<Output = ()>>;
// type ConnFun = Box<fn(&mut Context) -> dyn Future<Output = ()>>;
// type ConnFun = Box<dyn Fn(&'static mut Context) -> BoxFuture<'static, ()> + Send + 'static>;
// type ConnFun = Box<dyn Fn(&mut Context) -> BoxFuture<'static,()>;
// pub type ConnFun = AsyncFnPtr<()>;

struct AsyncFnPtr {
    func: Box<dyn Fn(crate::Context) -> BoxFuture<'static, io::Result<()>> + Send + Sync + 'static>,
}

pub const ResCodeOk: i32 = 1;
pub const ResCodeErr: i32 = 2;
pub const ResCodeAuth: i32 = 3;
pub const ResCodeNotFound: i32 = 4;

// #[macro_export]
/* #[proc_macro_attribute]
pub fn controller(args: TokenStream, input: TokenStream) -> TokenStream {
    // parse the input
    let input = parse_macro_input!(input as ItemFn);
    // parse the arguments
    let mut args = parse_macro_input!(args as Args);
} */

#[derive(Clone)]
pub struct Engine {
    inner: ruisutil::ArcMut<Inner>,
}
struct Inner {
    ctx: ruisutil::Context,
    lmt_tm: LmtTmConfig,
    lmt_max: LmtMaxConfig,
    fns: RwLock<HashMap<i32, LinkedList<AsyncFnPtr>>>,
    lmts: RwLock<HashMap<i32, LmtMaxConfig>>,
    addr: String,
    lsr: Option<TcpListener>,
}
// unsafe impl Send for Engine {}
// unsafe impl Sync for Engine {}
// unsafe impl Send for AsyncFnPtr {}
// unsafe impl Sync for AsyncFnPtr {}
impl Drop for Inner {
    fn drop(&mut self) {
        self.lsr = None;
        self.ctx.stop();
        //self.lsr.
    }
}
impl Engine {
    pub fn new(ctx: Option<ruisutil::Context>, addr: &str) -> Self {
        Self {
            inner: ruisutil::ArcMut::new(Inner {
                ctx: ruisutil::Context::background(ctx),
                fns: RwLock::new(HashMap::new()),
                lmts: RwLock::new(HashMap::new()),
                addr: String::from(addr),
                lsr: None,
                lmt_tm: LmtTmConfig::default(),
                lmt_max: LmtMaxConfig::default(),
            }),
        }
    }
    pub fn set_lmt_tm(&self, limit: LmtTmConfig) {
        unsafe { self.inner.muts().lmt_tm = limit };
    }
    pub fn set_lmt_max(&self, limit: LmtMaxConfig) {
        unsafe { self.inner.muts().lmt_max = limit };
    }
    pub fn stop(&self) {
        unsafe { self.inner.muts().lsr = None };
        self.inner.ctx.stop();
    }
    pub async fn run(self) -> io::Result<()> {
        let lsr = TcpListener::bind(self.inner.addr.as_str()).await?;
        unsafe { self.inner.muts().lsr = Some(lsr) };
        let c = self.clone();
        task::spawn(async move {
            c.runs().await;
        });

        // self.runs().await;
        while !self.inner.ctx.done() {
            task::sleep(Duration::from_millis(100)).await;
        }
        Ok(())
    }
    async fn runs(&self) {
        if let Some(lsr) = &self.inner.lsr {
            let mut incom = lsr.incoming();
            while !self.inner.ctx.done() {
                match incom.next().await {
                    None => break,
                    Some(stream) => {
                        if let Ok(conn) = stream {
                            let c = self.clone();
                            task::spawn(async move {
                                c.run_cli(conn).await;
                                // task::block_on(c.run_cli(conn));
                            });
                        } else {
                            println!("stream conn err!!!!")
                        }
                    }
                }
            }
        }
    }

    pub async fn get_lmt_tm(&self) -> &LmtTmConfig {
        &self.inner.lmt_tm
    }
    pub async fn get_lmt_max(&self, k: i32) -> LmtMaxConfig {
        let lkv = self.inner.lmts.read().await;
        match lkv.get(&k) {
            Some(v) => v.clone(),
            None => self.inner.lmt_max.clone(),
        }
    }
    async fn run_cli(self, conn: TcpStream) {
        match Context::parse_conn(&self.inner.ctx, &self, conn).await {
            Err(e) => println!("ParseContext err:{}", e),
            Ok(res) => {
                // println!("control:{}", res.control());
                let mut fncs = None;
                {
                    let lkv = self.inner.fns.read().await;
                    if let Some(ls) = lkv.get(&res.control()) {
                        let mut vs = Vec::with_capacity(ls.len());
                        let mut itr = ls.iter();
                        while let Some(f) = itr.next() {
                            let fnc = &f.func;
                            vs.push(fnc(res.clone()))
                        }
                        fncs = Some(vs);
                    }
                }
                if let Some(ls) = fncs {
                    for ft in ls {
                        if res.is_sended() {
                            break;
                        }
                        if let Err(e) = ft.await {
                            if let Err(e) = res
                                .res_string(
                                    ResCodeErr,
                                    format!("method return err:{:?}", e).as_str(),
                                )
                                .await
                            {
                                println!("res_string method err:{}", e.to_string().as_str());
                            }
                        }
                    }
                } else {
                    println!("not found function:{}", res.control())
                }
                if !res.is_sended() {
                    if let Err(e) = res.res_string(ResCodeErr, "Unknown").await {
                        println!("res_string Unknown err:{}", e.to_string().as_str());
                    }
                }
            }
        }
    }
    // pub fn reg_fun(&mut self, control: i32, f: AsyncFnPtr) {
    pub async fn reg_fun<F>(&self, control: i32, f: fn(Context) -> F, lmto: Option<LmtMaxConfig>)
    where
        F: Future<Output = io::Result<()>> + Send + 'static,
    {
        // fun(&mut Context::new(1));
        let fnc = AsyncFnPtr {
            func: Box::new(move |c: Context| Box::pin(f(c))),
        };
        {
            let mut lkv = self.inner.fns.write().await;
            if let Some(v) = lkv.get_mut(&control) {
                v.push_back(fnc);
            } else {
                let mut v = LinkedList::new();
                v.push_back(fnc);
                lkv.insert(control, v);
            }
        }
        if let Some(v) = lmto {
            let mut lkv = self.inner.lmts.write().await;
            lkv.insert(control, v);
        }
    }
}
