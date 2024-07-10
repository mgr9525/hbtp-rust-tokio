mod msger;
// mod msgbuf;

use tokio::sync::mpsc::Sender;
pub use msger::Messager;
// pub use msgbuf::MessagBuffer;

use super::msg::{Message, Messages};
use futures::future::BoxFuture;

pub type Senders = Sender<Messages>;

pub trait MessageRecv {
    fn on_check(&self) -> BoxFuture<'static, ()>;
    fn on_msg(&self, msg: Message) -> BoxFuture<'static, std::io::Result<()>>;
    // fn on_msg(self, msg: msg::Message) -> Pin<Box<dyn Future<Output = ()> + Sync + Send+'static>>;
}
pub type TMessageRecv = dyn MessageRecv + Send + Sync;
