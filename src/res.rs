use std::{collections::HashMap, io, time::Duration};

use async_std::{net::TcpStream, sync::Mutex};
use qstring::QString;
use serde::{Deserialize, Serialize};

/* fn callfun(fun: &ConnFun, ctx: &mut Context) {
  std::panic::catch_unwind(|| println!("callfun catch panic"));
  fun(ctx);
} */

#[derive(Clone)]
pub struct Context {
    inner: ruisutil::ArcMut<Inner>,
}
struct Inner {
    sended: bool,
    conn: Option<TcpStream>,
    ctrl: i32,
    cmds: String,
    args: Option<QString>,
    heads: Option<ruisutil::bytes::ByteBox>,
    bodys: Option<ruisutil::bytes::ByteBox>,
    bodyok: Mutex<bool>,
    bodylen: usize,

    data: HashMap<String, Vec<u8>>,
}
impl<'a> Context {
    fn new(control: i32, byln: usize) -> Self {
        Self {
            inner: ruisutil::ArcMut::new(Inner {
                sended: false,
                conn: None,
                ctrl: control,
                cmds: String::new(),
                args: None,
                heads: None,
                bodys: None,
                bodyok: Mutex::new(false),
                bodylen: byln,
                data: HashMap::new(),
            }),
        }
    }

    pub async fn parse_conn(
        ctx: &ruisutil::Context,
        egn: &crate::Engine,
        mut conn: TcpStream,
    ) -> io::Result<Self> {
        let mut info = MsgInfo::new();
        let infoln = std::mem::size_of::<MsgInfo>();
        let lmt_tm = egn.get_lmt_tm().await;
        let ctxs = ruisutil::Context::with_timeout(Some(ctx.clone()), lmt_tm.tm_ohther);
        let bts = ruisutil::tcp_read_async(&ctxs, &mut conn, infoln).await?;
        ruisutil::byte2struct(&mut info, &bts[..])?;
        if info.version != 1 && info.version != 2 {
            return Err(ruisutil::ioerr("not found version!", None));
        }
        let cfg = egn.get_lmt_max(info.control).await;
        if (info.len_cmd + info.len_arg) as u64 > cfg.max_ohther {
            return Err(ruisutil::ioerr("bytes1 out limit!!", None));
        }
        if (info.len_head) as u64 > cfg.max_heads {
            return Err(ruisutil::ioerr("bytes2 out limit!!", None));
        }
        if info.version >= 2 {
            let bts = ruisutil::tcp_read_async(&ctxs, &mut conn, 4).await?;
            // 'H', 'B', 'T', 'P'
            // if bts[0] == 0x48 && bts[0] == 0x42 && bts[0] == 0x54 && bts[0] == 0x50 {
            if !bts[..].eq(&[0x48, 0x42, 0x54, 0x50]) {
                return Err(ruisutil::ioerr("HBTP fmt err!!", None));
            }
        }

        let rt = Self::new(info.control, info.len_body as usize);
        let ins = unsafe { rt.inner.muts() };
        let lnsz = info.len_cmd as usize;
        if lnsz > 0 {
            let bts = ruisutil::tcp_read_async(&ctxs, &mut conn, lnsz).await?;
            ins.cmds = match std::str::from_utf8(&bts[..]) {
                Err(_) => return Err(ruisutil::ioerr("cmd err", None)),
                Ok(v) => String::from(v),
            };
        }
        let lnsz = info.len_arg as usize;
        if lnsz > 0 {
            let bts = ruisutil::tcp_read_async(&ctxs, &mut conn, lnsz as usize).await?;
            let args = match std::str::from_utf8(&bts[..]) {
                Err(_) => return Err(ruisutil::ioerr("args err", None)),
                Ok(v) => String::from(v),
            };
            ins.args = Some(QString::from(args.as_str()));
        }
        let ctxs = ruisutil::Context::with_timeout(Some(ctx.clone()), lmt_tm.tm_heads);
        let lnsz = info.len_head as usize;
        if lnsz > 0 {
            let bts = ruisutil::tcp_read_async(&ctxs, &mut conn, lnsz as usize).await?;
            ins.heads = Some(ruisutil::bytes::ByteBox::from(bts));
        }
        /* let ctxs = ruisutil::Context::with_timeout(Some(ctx.clone()), lmt_tm.tm_bodys);
        let lnsz = info.len_body as usize;
        if lnsz > 0 {
            let bts = ruisutil::tcp_read_async(&ctxs, &mut conn, lnsz as usize).await?;
            ins.bodys = Some(bts);
        } */
        ins.conn = Some(conn);
        Ok(rt)
    }

    // ---------------------------------------------------------------------------------------------------------------

    pub fn get_data(&self, s: &str) -> Option<&Vec<u8>> {
        self.inner.data.get(&String::from(s))
    }
    pub fn put_data(&self, s: &str, v: Vec<u8>) {
        let ins = unsafe { self.inner.muts() };
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
    pub async fn own_conn(&self) -> TcpStream {
        self.get_bodys(None).await;
        let ins = unsafe { self.inner.muts() };
        if let Some(v) = std::mem::replace(&mut ins.conn, None) {
            return v;
        }
        panic!("conn?");
    }
    pub fn peer_addr(&self) -> io::Result<String> {
        if let Some(conn) = &self.inner.conn {
            let addr = conn.peer_addr()?;
            Ok(addr.to_string())
        } else {
            Err(ruisutil::ioerr("can't get addr", None))
        }
    }
    pub fn control(&self) -> i32 {
        self.inner.ctrl
    }
    pub fn command(&self) -> &str {
        self.inner.cmds.as_str()
    }
    pub fn get_args(&'a self) -> Option<&'a QString> {
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
        let ins = unsafe { self.inner.muts() };
        if let Some(v) = &mut ins.args {
            v.add_pair((name, value));
        } else {
            ins.args = Some(QString::new(vec![(name, value)]));
        }
    }
    pub fn get_heads(&self) -> &Option<ruisutil::bytes::ByteBox> {
        &self.inner.heads
    }
    pub async fn get_bodys(
        &self,
        ctx: Option<ruisutil::Context>,
    ) -> &Option<ruisutil::bytes::ByteBox> {
        let mut lkv = self.inner.bodyok.lock().await;
        if !*lkv {
            if self.inner.bodylen > 0 {
                let ins = unsafe { self.inner.muts() };
                if let Some(conn) = &mut ins.conn {
                    let ctxs = match ctx {
                        None => ruisutil::Context::background(None),
                        Some(v) => v,
                    };
                    match ruisutil::tcp_read_async(&ctxs, conn, self.inner.bodylen).await {
                        Ok(bts) => ins.bodys = Some(ruisutil::bytes::ByteBox::from(bts)),
                        Err(e) => println!("get_bodys tcp read err:{}", e),
                    }
                }
            }
            *lkv = true
        }
        &self.inner.bodys
    }
    pub fn body_len(&self) -> usize {
        self.inner.bodylen
    }
    pub fn is_sended(&self) -> bool {
        self.inner.sended
    }
    pub fn head_json<T: Deserialize<'a>>(&'a self) -> io::Result<T> {
        match &self.inner.heads {
            None => Err(ruisutil::ioerr("heads nil", None)),
            Some(v) => match serde_json::from_slice(v) {
                Ok(vs) => Ok(vs),
                Err(e) => Err(ruisutil::ioerr(e, None)),
            },
        }
    }
    pub async fn body_json<T: Deserialize<'a>>(&'a self) -> io::Result<T> {
        match self.get_bodys(None).await {
            None => Err(ruisutil::ioerr("bodys nil", None)),
            Some(v) => match serde_json::from_slice(v) {
                Ok(vs) => Ok(vs),
                Err(e) => Err(ruisutil::ioerr(e, None)),
            },
        }
    }

    pub async fn response(
        &self,
        code: i32,
        hds: Option<&[u8]>,
        bds: Option<&[u8]>,
    ) -> io::Result<()> {
        /* let conn = match &mut self.conn {
            Some(v) => v,
            None => return Err(ruisutil::ioerr("not found conn", None)),
        }; */
        if let None = self.inner.conn {
            return Err(ruisutil::ioerr("not found conn", None));
        }
        if self.inner.sended {
            return Err(ruisutil::ioerr("already responsed!", None));
        }
        let ins = unsafe { self.inner.muts() };
        ins.sended = true;
        let mut res = ResInfoV1::new();
        res.code = code;
        if let Some(v) = hds {
            res.len_head = v.len() as u32;
        }
        if let Some(v) = bds {
            res.len_body = v.len() as u32;
        }
        if let Some(conn) = &mut ins.conn {
            let bts = ruisutil::struct2byte(&res);
            let ctx = ruisutil::Context::with_timeout(None, Duration::from_secs(10));
            ruisutil::tcp_write_async(&ctx, conn, bts).await?;
            if let Some(v) = hds {
                let ctx = ruisutil::Context::with_timeout(None, Duration::from_secs(20));
                ruisutil::tcp_write_async(&ctx, conn, v).await?;
            }
            if let Some(v) = bds {
                let ctx = ruisutil::Context::with_timeout(None, Duration::from_secs(30));
                ruisutil::tcp_write_async(&ctx, conn, v).await?;
            }
        } else {
            return Err(ruisutil::ioerr("not found conn", None));
        }

        Ok(())
    }
    pub async fn res_bytes(&self, code: i32, bds: &[u8]) -> io::Result<()> {
        self.response(code, None, Some(bds)).await
    }
    pub async fn res_string(&self, code: i32, s: &str) -> io::Result<()> {
        self.res_bytes(code, s.as_bytes()).await
    }
    pub async fn res_json<T: Serialize>(&self, code: i32, v: &T) -> io::Result<()> {
        match serde_json::to_string(v) {
            Ok(body) => self.res_string(code, body.as_str()).await,
            Err(e) => Err(ruisutil::ioerr(format!("json format err:{}", e), None)),
        }
    }
}

//----------------------------------bean
#[repr(C, packed)]
pub struct MsgInfo {
    pub version: u16,
    pub control: i32,
    pub len_cmd: u16,
    pub len_arg: u16,
    pub len_head: u32,
    pub len_body: u32,
}
impl MsgInfo {
    pub fn new() -> Self {
        Self {
            version: 0,
            control: 0,
            len_cmd: 0,
            len_arg: 0,
            len_head: 0,
            len_body: 0,
        }
    }
}
#[repr(C, packed)]
pub struct ResInfoV1 {
    pub code: i32,
    pub len_head: u32,
    pub len_body: u32,
}
impl ResInfoV1 {
    pub fn new() -> Self {
        Self {
            code: 0,
            len_head: 0,
            len_body: 0,
        }
    }
}

#[derive(Clone)]
pub struct LmtMaxConfig {
    pub max_ohther: u64,
    pub max_heads: u64,
}

impl Default for LmtMaxConfig {
    fn default() -> Self {
        Self {
            max_ohther: 1024 * 1024 * 2, //2M
            max_heads: 1024 * 1024 * 10, //10M
        }
    }
}

#[derive(Clone)]
pub struct LmtTmConfig {
    pub tm_ohther: Duration,
    pub tm_heads: Duration,
    pub tm_bodys: Duration,
}

impl Default for LmtTmConfig {
    fn default() -> Self {
        Self {
            tm_ohther: Duration::from_secs(10),
            tm_heads: Duration::from_secs(30),
            tm_bodys: Duration::from_secs(50),
        }
    }
}
