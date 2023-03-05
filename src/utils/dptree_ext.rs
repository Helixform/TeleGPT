use teloxide::prelude::*;
use teloxide::types::Me;

pub trait HandlerExt {
    fn post_chain(self, next: Self) -> Self;
}

impl<'a, Input, Output, Descr> HandlerExt for Handler<'a, Input, Output, Descr>
where
    Input: Clone + Send + 'a,
    Output: 'a,
    Descr: dptree::HandlerDescription,
{
    fn post_chain(self, next: Self) -> Self {
        dptree::from_fn(move |input: Input, cont| {
            let handler = self.clone();
            let next = next.clone();
            let input_clone = input.clone();
            async {
                let _ = handler
                    .execute(
                        input_clone,
                        |event| async move { ControlFlow::Continue(event) },
                    )
                    .await;
                next.execute(input, cont).await
            }
        })
    }
}

pub fn command_filter(cmd: &'static str) -> impl Fn(Message, Me) -> bool {
    move |msg: Message, me: Me| {
        let text = msg.text();
        if text.is_none() {
            return false;
        }
        let text = text.unwrap();

        let pat = format!("/{}", cmd);
        if !text.starts_with(&pat) {
            return false;
        }

        // When sending commands in a group, a mention suffix may be attached to
        // the text. For example: "/reset@xxxx_bot".
        let rest = &text[pat.len()..];
        if rest.len() > 1 {
            return me
                .username
                .as_ref()
                .map(|n| n == &rest[1..])
                .unwrap_or(false);
        }

        true
    }
}
