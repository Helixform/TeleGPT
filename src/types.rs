use anyhow::Error;
use teloxide::dispatching::DefaultKey;
use teloxide::dispatching::DpHandlerDescription;
use teloxide::prelude::*;

pub(crate) type HandlerResult = Result<(), Error>;
pub(crate) type TeloxideHandler =
    Handler<'static, DependencyMap, HandlerResult, DpHandlerDescription>;
pub(crate) type TeloxideDispatcher = Dispatcher<Bot, Error, DefaultKey>;
