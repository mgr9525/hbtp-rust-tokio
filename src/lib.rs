// extern crate proc_macro;
extern crate async_std;
extern crate futures;
extern crate qstring;

use std::{
    any,
    borrow::{BorrowMut, Cow},
    collections::{HashMap, LinkedList},
    io, mem,
    pin::Pin,
    ptr,
    sync::{Arc, RwLock},
    thread,
    time::{Duration, SystemTime},
    usize,
};

use async_std::prelude::*;
use async_std::{
    net::{TcpListener, TcpStream},
    task,
};
use futures::future::{BoxFuture, Future, FutureExt};
use qstring::QString;
pub use req::Request;
pub use req::Response;
pub use res::Context;
// pub use res::CtxInner;

mod req;
mod res;
pub mod util;

#[cfg(test)]
mod tests {
    use std::{future::Future, mem, pin::Pin, sync::Arc, thread};

    use async_std::task;
    use futures::future::FutureExt;
    use qstring::QString;

    use crate::{util, AsyncFnPtr, Engine, Request};

    /* #[test]
    fn it_works() {
        println!("hello test");
        let ctx1 = util::Context::background(None);
        let ctx2 = util::Context::background(Some(ctx1.clone()));
        println!("start:ctx1:{},ctx2:{}", ctx1.done(), ctx2.done());
        ctx2.stop();
        println!("end:ctx1:{},ctx2:{}", ctx1.done(), ctx2.done());

        let wg = util::WaitGroup::new();
        let wg1 = wg.clone();
        thread::spawn(move || {
            let mut info = MsgInfo::new();
            info.version = 1;
            info.control = 2;
            info.lenCmd = 1000;
            let bts = util::struct2byte(&info);
            let ln = std::mem::size_of::<MsgInfo>();
            println!(
                "MsgInfo info.v:{},bts({}/{}):{:?}",
                info.version,
                bts.len(),
                ln,
                bts
            );
            let mut infos = MsgInfo::new();
            if let Ok(()) = util::byte2struct(&mut infos, bts) {
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
        serv.reg_fun(1, testFun);
        task::block_on(Engine::run(serv));
    }
    async fn testFun(c: crate::Context) {
        println!(
            "testFun ctrl:{},cmd:{},ishell:{},arg hello1:{}",
            c.control().await,
            c.command().await,
            c.command().await == "hello",
            c.get_arg("hehe1").await.unwrap().as_str()
        );
        if let Some(v) = c.get() {
            let cs = v.read().await;
            cs.get_bodys();
        }
        // panic!("whats?");
        if let Err(e) = c
            .res_string(crate::ResCodeOk, "hello,there is rust!!")
            .await
        {
            println!("testFun res_string err:{}", e)
        };
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
    func: Box<dyn Fn(crate::Context) -> BoxFuture<'static, ()> + Send + Sync + 'static>,
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
    inner: Arc<Inner>,
    ptr: u64,
}
struct Inner {
    ctx: util::Context,
    fns: RwLock<HashMap<i32, LinkedList<AsyncFnPtr>>>,
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
    pub fn new(ctx: Option<util::Context>, addr: &str) -> Self {
        let inr = Arc::new(Inner {
            ctx: util::Context::background(ctx),
            fns: RwLock::new(HashMap::new()),
            addr: String::from(addr),
            lsr: None,
        });
        Self {
            ptr: (&*inr) as *const Inner as u64,
            inner: inr,
        }
    }
    unsafe fn inners<'a>(&'a self) -> &'a mut Inner {
        &mut *(self.ptr as *mut Inner)
    }
    pub fn stop(&self) {
        unsafe { self.inners().lsr = None };
        self.inner.ctx.stop();
    }
    pub async fn run(self) -> io::Result<()> {
        let lsr = TcpListener::bind(self.inner.addr.as_str()).await?;
        unsafe { self.inners().lsr = Some(lsr) };
        let c = self.clone();
        task::spawn(async move {
            task::block_on(Self::runs(c));
        });

        while !self.inner.ctx.done() {
            task::sleep(Duration::from_millis(100)).await;
        }
        Ok(())
    }
    async fn runs(self) {
        if let Some(lsr) = &self.inner.lsr {
            let mut incom = lsr.incoming();
            while !self.inner.ctx.done() {
                match incom.next().await {
                    None => break,
                    Some(stream) => {
                        if let Ok(conn) = stream {
                            let c = self.clone();
                            task::spawn(async move {
                                task::block_on(Self::run_cli(c, conn));
                            });
                        } else {
                            println!("stream conn err!!!!")
                        }
                    }
                }
            }
        }
    }
    async fn run_cli(self, mut conn: TcpStream) {
        match res::ParseContext(&self.inner.ctx, conn).await {
            Err(e) => println!("ParseContext err:{}", e),
            Ok(mut res) => {
                println!("control:{}", res.control().await);
                if let Ok(lkv) = self.inner.fns.read() {
                    if let Some(ls) = lkv.get(&res.control().await) {
                        let mut itr = ls.iter();
                        while let Some(f) = itr.next() {
                            if res.is_sended().await {
                                break;
                            }
                            let fnc = &f.func;
                            fnc(res.clone()).await;
                            // task::spawn(fpn.run(rtx)).await;
                        }

                        if !res.is_sended().await {
                            res.res_string(ResCodeErr, "Unknown").await;
                        }
                    } else {
                        println!("not found function:{}", res.control().await)
                    }
                }
            }
        }
    }
    // pub fn reg_fun(&mut self, control: i32, f: AsyncFnPtr) {
    pub fn reg_fun<F>(&self, control: i32, f: fn(Context) -> F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        // fun(&mut Context::new(1));
        let fnc = AsyncFnPtr {
            func: Box::new(move |c: Context| Box::pin(f(c))),
        };
        if let Ok(mut lkv) = self.inner.fns.write() {
            if let Some(v) = lkv.get_mut(&control) {
                v.push_back(fnc);
            } else {
                let mut v = LinkedList::new();
                v.push_back(fnc);
                lkv.insert(control, v);
            }
        }
    }
}
