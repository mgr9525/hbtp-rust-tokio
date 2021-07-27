use std::{collections::HashMap, io, mem, sync::Arc, time::Duration};

use async_std::{
    net::TcpStream,
    sync::{RwLock, RwLockReadGuard},
};
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
    let mut rt = CtxInner::new(info.control);
    let lnsz = info.lenCmd as usize;
    if lnsz > 0 {
        let bts = util::tcp_read(&ctxs, &mut conn, lnsz).await?;
        rt.cmds = match std::str::from_utf8(&bts[..]) {
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
        rt.args = Some(QString::from(args.as_str()));
    }
    let ctxs = util::Context::with_timeout(Some(ctx.clone()), Duration::from_secs(30));
    let lnsz = info.lenHead as usize;
    if lnsz > 0 {
        let bts = util::tcp_read(&ctxs, &mut conn, lnsz as usize).await?;
        rt.heads = Some(bts);
    }
    let ctxs = util::Context::with_timeout(Some(ctx.clone()), Duration::from_secs(50));
    let lnsz = info.lenBody as usize;
    if lnsz > 0 {
        let bts = util::tcp_read(&ctxs, &mut conn, lnsz as usize).await?;
        rt.bodys = Some(bts);
    }
    rt.conn = Some(conn);
    Ok(Context::new(rt))
}

/* fn callfun(fun: &ConnFun, ctx: &mut Context) {
  std::panic::catch_unwind(|| println!("callfun catch panic"));
  fun(ctx);
} */

#[derive(Clone)]
pub struct Context {
    pub inner: Arc<RwLock<CtxInner>>,
}
pub struct CtxInner {
    sended: bool,
    conn: Option<TcpStream>,
    ctrl: i32,
    cmds: String,
    args: Option<QString>,
    heads: Option<Box<[u8]>>,
    bodys: Option<Box<[u8]>>,

    data: HashMap<String, Vec<u8>>,
}
impl CtxInner {
    pub fn new(control: i32) -> Self {
        Self {
            sended: false,
            conn: None,
            ctrl: control,
            cmds: String::new(),
            args: None,
            heads: None,
            bodys: None,
            data: HashMap::new(),
        }
    }
}
impl Context {
    fn new(inr: CtxInner) -> Self {
        Self {
            inner: Arc::new(RwLock::new(inr)),
        }
    }

    pub async fn get_data(&self, s: &str) -> Option<Vec<u8>> {
        let this = self.inner.read().await;
        if let Some(v) = this.data.get(&String::from(s)) {
            return Some(v.clone());
        }
        None
    }
    pub async fn put_data(&self, s: &str, v: Vec<u8>) {
        let mut this = self.inner.write().await;
        this.data.insert(String::from(s), v);
    }
    /* pub fn get_conn(&self) -> &TcpStream {
        if let Ok(this) = self.inner.read() {
            if let Some(v) = &this.conn {
                return v;
            }
        }
        panic!("conn?");
    } */
    pub async fn own_conn(&self) -> TcpStream {
        let mut this = self.inner.write().await;
        if let Some(v) = std::mem::replace(&mut this.conn, None) {
            return v;
        }
        panic!("conn?");
    }
    pub async fn control(&self) -> i32 {
        let this = self.inner.read().await;
        this.ctrl
    }
    pub async fn command(&self) -> String {
        let this = self.inner.read().await;
        this.cmds.clone()
    }
    pub async fn get_args(&self) -> Option<QString> {
        let this = self.inner.read().await;
        if let Some(v) = &this.args {
            return Some(v.clone());
        }
        None
    }
    pub async fn get_arg(&self, name: &str) -> Option<String> {
        let this = self.inner.read().await;
        if let Some(v) = &this.args {
            if let Some(s) = v.get(name) {
                return Some(String::from(s));
            }
        }
        None
    }
    /* pub fn set_arg(&self, name: &str, value: &str) {
        if let None = &self.args {
            self.args = Some(QString::from(""));
        }
        self.args.unwrap().add_str(origin)
    } */
    pub async fn add_arg(&self, name: &str, value: &str) {
        let mut this = self.inner.write().await;
        if let Some(v) = &mut this.args {
            v.add_pair((name, value));
        } else {
            this.args = Some(QString::new(vec![(name, value)]));
        }
    }
    pub fn get<'a>(&'a self) -> Option<&'a Arc<RwLock<CtxInner>>> {
        Some(&self.inner)
    }
    pub async fn get_heads(&self) -> Option<Box<[u8]>> {
        let this = self.inner.read().await;
        if let Some(v) = &this.heads {
            return Some(v.clone());
        }
        None
    }
    pub async fn get_bodys(&self) -> Option<Box<[u8]>> {
        let this = self.inner.read().await;
        if let Some(v) = &this.bodys {
            return Some(v.clone());
        }
        None
    }
    pub async fn is_sended(&self) -> bool {
        let this = self.inner.read().await;
        this.sended
    }

    pub async fn response(
        &self,
        code: i32,
        hds: Option<&[u8]>,
        bds: Option<&[u8]>,
    ) -> io::Result<()> {
        let mut this = self.inner.write().await;
        /* let conn = match &mut self.conn {
            Some(v) => v,
            None => return Err(util::ioerrs("not found conn", None)),
        }; */
        if let None = this.conn {
            return Err(util::ioerrs("not found conn", None));
        }
        if this.sended {
            return Err(util::ioerrs("already responsed!", None));
        }
        this.sended = true;
        let mut res = ResInfoV1::new();
        res.code = code;
        if let Some(v) = hds {
            res.lenHead = v.len() as u32;
        }
        if let Some(v) = bds {
            res.lenBody = v.len() as u32;
        }
        if let Some(conn) = &mut this.conn {
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
impl CtxInner {
    /* pub fn get_data(&self, s: &str) -> Option<&Vec<u8>> {
        self.data.get(&String::from(s))
    }
    pub fn put_data(&mut self, s: &str, v: Vec<u8>) {
        self.data.insert(String::from(s), v);
    } */
    pub fn get_conn(&self) -> &TcpStream {
        if let Some(v) = &self.conn {
            return v;
        }
        panic!("conn?");
    }
    pub fn own_conn(&mut self) -> TcpStream {
        if let Some(v) = std::mem::replace(&mut self.conn, None) {
            return v;
        }
        panic!("conn?");
    }
    pub fn control(&self) -> i32 {
        self.ctrl
    }
    pub fn command(&self) -> &str {
        self.cmds.as_str()
    }
    pub fn get_args<'a>(&'a self) -> Option<&'a QString> {
        if let Some(v) = &self.args {
            Some(v)
        } else {
            None
        }
    }
    pub fn get_arg(&self, name: &str) -> Option<String> {
        if let Some(v) = &self.args {
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
        if let Some(v) = &mut self.args {
            v.add_pair((name, value));
        } else {
            self.args = Some(QString::new(vec![(name, value)]));
        }
    }
    pub fn get_heads(&self) -> &Option<Box<[u8]>> {
        &self.heads
    }
    pub fn get_bodys(&self) -> &Option<Box<[u8]>> {
        &self.bodys
    }
    pub fn is_sended(&self) -> bool {
        self.sended
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
