use std::{collections::HashMap, io, mem, sync::Arc, time::Duration};

use async_std::net::TcpStream;
use qstring::QString;

use crate::util;

pub const MaxOther: u64 = 1024 * 1024 * 20; //20M
pub const MaxHeads: u64 = 1024 * 1024 * 100; //100M
pub const MaxBodys: u64 = 1024 * 1024 * 1024; //1G

pub async fn ParseContext(ctx: &util::Context, mut conn: TcpStream) -> io::Result<Context> {
    let mut info = MsgInfo::new();
    let infoln = mem::size_of::<MsgInfo>();
    let ctxs = util::Context::with_timeout(Some(ctx.clone()), Duration::from_secs(10));
    let bts = util::tcp_read(&ctxs, &mut conn, infoln).await?;
    util::byte2struct(&mut info, &bts[..])?;
    if info.version != 1 {
        return Err(util::ioerrs("not found version!", None));
    }
    if (info.lenCmd + info.lenArg) as u64 > MaxOther {
        return Err(util::ioerrs("bytes1 out limit!!", None));
    }
    if (info.lenHead) as u64 > MaxHeads {
        return Err(util::ioerrs("bytes2 out limit!!", None));
    }
    if (info.lenBody) as u64 > MaxBodys {
        return Err(util::ioerrs("bytes3 out limit!!", None));
    }
    let mut rt = Context::new(info.control);
    let ins = unsafe { rt.inners() };
    let lnsz = info.lenCmd as usize;
    if lnsz > 0 {
        let bts = util::tcp_read(&ctxs, &mut conn, lnsz).await?;
        ins.cmds = match std::str::from_utf8(&bts[..]) {
            Err(e) => return Err(util::ioerrs("cmd err", None)),
            Ok(v) => String::from(v),
        };
    }
    let lnsz = info.lenArg as usize;
    if lnsz > 0 {
        let bts = util::tcp_read(&ctxs, &mut conn, lnsz as usize).await?;
        let args = match std::str::from_utf8(&bts[..]) {
            Err(e) => return Err(util::ioerrs("args err", None)),
            Ok(v) => String::from(v),
        };
        ins.args = Some(QString::from(args.as_str()));
    }
    let ctxs = util::Context::with_timeout(Some(ctx.clone()), Duration::from_secs(30));
    let lnsz = info.lenHead as usize;
    if lnsz > 0 {
        let bts = util::tcp_read(&ctxs, &mut conn, lnsz as usize).await?;
        ins.heads = Some(bts);
    }
    let ctxs = util::Context::with_timeout(Some(ctx.clone()), Duration::from_secs(50));
    let lnsz = info.lenBody as usize;
    if lnsz > 0 {
        let bts = util::tcp_read(&ctxs, &mut conn, lnsz as usize).await?;
        ins.bodys = Some(bts);
    }
    ins.conn = Some(conn);
    Ok(rt)
}

/* fn callfun(fun: &ConnFun, ctx: &mut Context) {
  std::panic::catch_unwind(|| println!("callfun catch panic"));
  fun(ctx);
} */

#[derive(Clone)]
pub struct Context {
    ptr: u64,
    inner: Arc<CtxInner>,
}
struct CtxInner {
    sended: bool,
    conn: Option<TcpStream>,
    ctrl: i32,
    cmds: String,
    args: Option<QString>,
    heads: Option<Box<[u8]>>,
    bodys: Option<Box<[u8]>>,

    data: HashMap<String, Vec<u8>>,
}
impl Context {
    fn new(control: i32) -> Self {
        let inr = Arc::new(CtxInner {
            sended: false,
            conn: None,
            ctrl: control,
            cmds: String::new(),
            args: None,
            heads: None,
            bodys: None,
            data: HashMap::new(),
        });
        Self {
            ptr: (&*inr) as *const CtxInner as u64,
            inner: inr,
        }
    }
    unsafe fn inners<'a>(&'a self) -> &'a mut CtxInner {
        &mut *(self.ptr as *mut CtxInner)
    }

    pub fn get_data(&self, s: &str) -> Option<&Vec<u8>> {
        self.inner.data.get(&String::from(s))
    }
    pub fn put_data(&self, s: &str, v: Vec<u8>) {
        let ins = unsafe { self.inners() };
        ins.data.insert(String::from(s), v);
    }

    /* pub fn get_conn(&self) -> &TcpStream {
        if let Ok(this) = self.inner.read() {
            if let Some(v) = &this.conn {
                return v;
            }
        }
        panic!("conn?");
    } */
    pub fn own_conn(&self) -> TcpStream {
        let ins = unsafe { self.inners() };
        if let Some(v) = std::mem::replace(&mut ins.conn, None) {
            return v;
        }
        panic!("conn?");
    }
    pub fn control(&self) -> i32 {
        self.inner.ctrl
    }
    pub fn command(&self) -> &str {
        self.inner.cmds.as_str()
    }
    pub fn get_args<'a>(&'a self) -> Option<&'a QString> {
        if let Some(v) = &self.inner.args {
            Some(v)
        } else {
            None
        }
    }
    pub fn get_arg(&self, name: &str) -> Option<String> {
        if let Some(v) = &self.inner.args {
            if let Some(s) = v.get(name) {
                Some(String::from(s))
            } else {
                None
            }
        } else {
            None
        }
    }
    /* pub fn set_arg(&mut self, name: &str, value: &str) {
        if let None = &self.args {
            self.args = Some(QString::from(""));
        }
        self.args.unwrap().add_str(origin)
    } */
    pub fn add_arg(&mut self, name: &str, value: &str) {
        let ins = unsafe { self.inners() };
        if let Some(v) = &mut ins.args {
            v.add_pair((name, value));
        } else {
            ins.args = Some(QString::new(vec![(name, value)]));
        }
    }
    pub fn get_heads(&self) -> &Option<Box<[u8]>> {
        &self.inner.heads
    }
    pub fn get_bodys(&self) -> &Option<Box<[u8]>> {
        &self.inner.bodys
    }
    pub fn own_heads(&self) -> Option<Box<[u8]>> {
        let ins = unsafe { self.inners() };
        std::mem::replace(&mut ins.heads, None)
    }
    pub fn own_bodys(&self) -> Option<Box<[u8]>> {
        let ins = unsafe { self.inners() };
        std::mem::replace(&mut ins.bodys, None)
    }
    pub fn is_sended(&self) -> bool {
        self.inner.sended
    }

    pub async fn response(
        &self,
        code: i32,
        hds: Option<&[u8]>,
        bds: Option<&[u8]>,
    ) -> io::Result<()> {
        /* let conn = match &mut self.conn {
            Some(v) => v,
            None => return Err(util::ioerrs("not found conn", None)),
        }; */
        if let None = self.inner.conn {
            return Err(util::ioerrs("not found conn", None));
        }
        if self.inner.sended {
            return Err(util::ioerrs("already responsed!", None));
        }
        let ins = unsafe { self.inners() };
        ins.sended = true;
        let mut res = ResInfoV1::new();
        res.code = code;
        if let Some(v) = hds {
            res.lenHead = v.len() as u32;
        }
        if let Some(v) = bds {
            res.lenBody = v.len() as u32;
        }
        if let Some(conn) = &mut ins.conn {
            let bts = util::struct2byte(&res);
            let ctx = util::Context::with_timeout(None, Duration::from_secs(10));
            util::tcp_write(&ctx, conn, bts).await?;
            if let Some(v) = hds {
                let ctx = util::Context::with_timeout(None, Duration::from_secs(20));
                util::tcp_write(&ctx, conn, v).await?;
            }
            if let Some(v) = bds {
                let ctx = util::Context::with_timeout(None, Duration::from_secs(30));
                util::tcp_write(&ctx, conn, v).await?;
            }
        } else {
            return Err(util::ioerrs("not found conn", None));
        }

        Ok(())
    }
    pub async fn res_bytes(&self, code: i32, bds: &[u8]) -> io::Result<()> {
        self.response(code, None, Some(bds)).await
    }
    pub async fn res_string(&self, code: i32, s: &str) -> io::Result<()> {
        self.res_bytes(code, s.as_bytes()).await
    }
}

//----------------------------------bean
#[repr(C, packed)]
pub struct MsgInfo {
    pub version: u16,
    pub control: i32,
    pub lenCmd: u16,
    pub lenArg: u16,
    pub lenHead: u32,
    pub lenBody: u32,
}
impl MsgInfo {
    pub fn new() -> Self {
        Self {
            version: 0,
            control: 0,
            lenCmd: 0,
            lenArg: 0,
            lenHead: 0,
            lenBody: 0,
        }
    }
}
#[repr(C, packed)]
pub struct ResInfoV1 {
    pub code: i32,
    pub lenHead: u32,
    pub lenBody: u32,
}
impl ResInfoV1 {
    pub fn new() -> Self {
        Self {
            code: 0,
            lenHead: 0,
            lenBody: 0,
        }
    }
}
