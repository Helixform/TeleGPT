use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use futures::{Future, Stream, StreamExt as FuturesStreamExt};
use pin_project_lite::pin_project;

pin_project! {
    /// Stream for the [`throttle_buffer`](StreamExt::throttle_buffer) method.
    #[must_use = "streams do nothing unless polled"]
    pub struct ThrottleBuffer<St, B>
        where St: Stream,
    {
        #[pin]
        stream: St,
        interval: Duration,
        buffer: Option<B>,
        #[pin]
        active_sleep: Option<Box<dyn Future<Output = ()>>>,
        done: bool,
    }
}

unsafe impl<St, B> Send for ThrottleBuffer<St, B> where St: Stream {}

impl<St, B> ThrottleBuffer<St, B>
where
    St: Stream,
{
    fn new(stream: St, interval: Duration) -> Self {
        Self {
            stream,
            interval,
            buffer: None,
            active_sleep: None,
            done: false,
        }
    }
}

impl<St, B> Stream for ThrottleBuffer<St, B>
where
    St: Stream,
    B: Default + Extend<St::Item>,
{
    type Item = B;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        if *this.done {
            if let Some(buffer) = this.buffer.take() {
                // Deliver the rest buffer.
                return Poll::Ready(Some(buffer));
            }
            return Poll::Ready(None);
        }

        // Poll the stream until pending to ensure it's scheduled while being blocked by throttles.
        loop {
            match this.stream.as_mut().poll_next(cx) {
                Poll::Ready(Some(item)) => {
                    this.buffer.get_or_insert(Default::default()).extend([item]);
                }
                Poll::Ready(None) => {
                    *this.done = true;
                    break;
                }
                Poll::Pending => {
                    break;
                }
            }
        }

        if this.buffer.is_none() {
            // The stream is not ready yet, don't start throttling now.
            return Poll::Pending;
        }

        if let Some(sleep) = this.active_sleep.as_mut().as_pin_mut() {
            let sleep = unsafe { sleep.map_unchecked_mut(|s| s.as_mut()) };
            futures::ready!(sleep.poll(cx));
        }

        // Reset the outstanding `Sleep` every time after waking up from throttling.
        this.active_sleep
            .set(Some(Box::new(tokio::time::sleep(*this.interval))));

        Poll::Ready(Some(
            this.buffer
                .take()
                .expect("buffer should not be `None` here"),
        ))
    }
}

pub trait StreamExt: FuturesStreamExt {
    fn throttle_buffer<B>(self, interval: Duration) -> ThrottleBuffer<Self, B>
    where
        Self: Sized,
        B: Default + Extend<Self::Item>;
}

impl<S> StreamExt for S
where
    S: FuturesStreamExt,
{
    fn throttle_buffer<B>(self, interval: Duration) -> ThrottleBuffer<Self, B>
    where
        Self: Sized,
        B: Default + Extend<Self::Item>,
    {
        ThrottleBuffer::new(self, interval)
    }
}
