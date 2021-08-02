// use async_std::io::{ReadExt, Write};
use async_std::net;
use async_std::prelude::*;
use std::{
    io::{self, Read, Write},
    sync::{Arc, Condvar, Mutex},
    thread,
    time::{self, Duration},
};

pub fn Byte2I(bts: &[u8]) -> i64 {
    let mut rt = 0i64;
    let mut i = bts.len();
    for v in bts {
        if i > 0 {
            i -= 1;
            rt |= (*v as i64) << (8 * i);
        } else {
            rt |= *v as i64;
        }
    }
    rt
}

pub fn I2Byte(v: i64, n: usize) -> Box<[u8]> {
    let mut rt: Vec<u8> = Vec::with_capacity(n);
    // if n>4{return rt;}
    for i in 0..n {
        let k = n - i - 1;
        if k > 0 {
            rt.push((v >> (8 * k)) as u8);
        } else {
            rt.push(v as u8)
        }
    }
    rt.into_boxed_slice()
}

pub fn ioerrs(s: &str, kd: Option<io::ErrorKind>) -> io::Error {
    let mut kds = io::ErrorKind::Other;
    if let Some(v) = kd {
        kds = v;
    }
    io::Error::new(kds, s)
}
pub fn struct2byte<T: Sized>(p: &T) -> &[u8] {
    unsafe { std::slice::from_raw_parts((p as *const T) as *const u8, std::mem::size_of::<T>()) }
}
pub fn byte2struct<T: Sized>(p: &mut T, bts: &[u8]) -> io::Result<()> {
    let ln = std::mem::size_of::<T>();
    if ln != bts.len() {
        return Err(ioerrs("param err!", None));
    }

    unsafe {
        let ptr = p as *mut T as *mut u8;
        let tb = bts.as_ptr();
        std::ptr::copy_nonoverlapping(tb, ptr, ln);
    };
    Ok(())
}

pub async fn tcp_read(
    ctx: &Context,
    stream: &mut net::TcpStream,
    ln: usize,
) -> io::Result<Box<[u8]>> {
    if ln <= 0 {
        return Ok(Box::new([0u8; 0]));
    }
    let mut rn = 0usize;
    let mut data = vec![0u8; ln];
    while rn < ln {
        if ctx.done() {
            return Err(io::Error::new(io::ErrorKind::Other, "ctx end!"));
        }
        match stream.read(&mut data[rn..]).await {
            Ok(n) => {
                if n > 0 {
                    rn += n;
                } else {
                    // let bts=&data[..];
                    // println!("read errs:ln:{},rn:{},n:{}，dataln:{}，bts:{}",ln,rn,n,data.len(),bts.len());
                    return Err(io::Error::new(io::ErrorKind::Other, "read err!"));
                }
            }
            Err(e) => return Err(e),
        }
    }
    Ok(data.into_boxed_slice())
}
pub async fn tcp_write(
    ctx: &Context,
    stream: &mut net::TcpStream,
    bts: &[u8],
) -> io::Result<usize> {
    if bts.len() <= 0 {
        return Ok(0);
    }
    if ctx.done() {
        return Err(io::Error::new(io::ErrorKind::Other, "ctx end!"));
    }
    match stream.write(bts).await {
        Err(e) => Err(e),
        Ok(n) => {
            if n != bts.len() {
                Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("send len err:{}/{}", n, bts.len()),
                ))
            } else {
                Ok(n)
            }
        }
    }
}

#[derive(Clone)]
pub struct Context {
    inner: Arc<CtxInner>,
}
struct CtxInner {
    parent: Option<Context>,
    doned: Mutex<bool>,

    times: time::SystemTime,
    timeout: Mutex<Option<time::Duration>>,
}

impl Context {
    pub fn background(prt: Option<Context>) -> Self {
        Self {
            inner: Arc::new(CtxInner {
                parent: prt,
                doned: Mutex::new(false),
                times: time::SystemTime::now(),
                timeout: Mutex::new(None),
            }),
        }
    }

    pub fn with_timeout(prt: Option<Context>, tmd: time::Duration) -> Self {
        let mut c = Self::background(prt);
        if let Ok(mut v) = c.inner.timeout.lock() {
            *v = Some(tmd)
        }
        c
    }

    pub fn done(&self) -> bool {
        if let Some(v) = &self.inner.parent {
            if v.done() {
                return true;
            }
        };
        if let Some(v) = &*self.inner.timeout.lock().unwrap() {
            if let Ok(vs) = time::SystemTime::now().duration_since(self.inner.times) {
                if vs.gt(v) {
                    return true;
                }
            }
        }
        *self.inner.doned.lock().unwrap()
    }

    pub fn stop(&self) -> bool {
        match self.inner.doned.lock() {
            Err(e) => false,
            Ok(mut v) => {
                *v = true;
                true
            }
        }
    }
}

pub struct WaitGroup {
    inner: Arc<WgInner>,
}

/// Inner state of a `WaitGroup`.
struct WgInner {
    count: Mutex<usize>,
}
impl WaitGroup {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(WgInner {
                count: Mutex::new(1),
            }),
        }
    }
    pub fn wait(&self) {
        loop {
            thread::sleep(Duration::from_millis(1));
            let mut count = self.inner.count.lock().unwrap();
            if *count <= 1 {
                break;
            }
        }
    }
}
impl Drop for WaitGroup {
    fn drop(&mut self) {
        if let Ok(mut v) = self.inner.count.lock() {
            *v -= 1;
        }
    }
}

impl Clone for WaitGroup {
    fn clone(&self) -> WaitGroup {
        if let Ok(mut v) = self.inner.count.lock() {
            *v += 1;
        }

        WaitGroup {
            inner: self.inner.clone(),
        }
    }
}