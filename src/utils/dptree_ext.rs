use teloxide::prelude::*;

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
