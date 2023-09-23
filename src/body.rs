use bytes::{Buf, Bytes};
use http_body::{Frame, SizeHint};
use std::convert::Infallible;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::sync::mpsc;

pin_project_lite::pin_project! {
    #[derive(Debug)]
    #[project = BodyProj]
    pub enum Body<T> {
        Empty,
        Buf {
            inner: Option<T>,
        },
        Bytes {
            inner: Option<Bytes>,
        },
        Stream {
            rx: mpsc::Receiver<Bytes>,
        },
    }
}

impl<T> From<Bytes> for Body<T> {
    fn from(bytes: Bytes) -> Self {
        Self::Bytes { inner: Some(bytes) }
    }
}

impl<T> From<mpsc::Receiver<Bytes>> for Body<T> {
    fn from(rx: mpsc::Receiver<Bytes>) -> Self {
        Self::Stream { rx }
    }
}

impl<T> Body<T> {
    pub fn new(buf: T) -> Self {
        Self::Buf { inner: Some(buf) }
    }

    pub fn from_static(bytes: &'static [u8]) -> Self {
        Self::from(Bytes::from_static(bytes))
    }
}

#[derive(Debug)]
pub enum BodyChunk<T: Buf> {
    Buf(T),
    Bytes(Bytes),
}

impl<T: Buf> bytes::Buf for BodyChunk<T> {
    fn remaining(&self) -> usize {
        match self {
            BodyChunk::Buf(inner) => inner.remaining(),
            BodyChunk::Bytes(inner) => inner.remaining(),
        }
    }

    fn chunk(&self) -> &[u8] {
        match self {
            BodyChunk::Buf(inner) => inner.chunk(),
            BodyChunk::Bytes(inner) => inner.chunk(),
        }
    }

    fn advance(&mut self, cnt: usize) {
        match self {
            BodyChunk::Buf(inner) => inner.advance(cnt),
            BodyChunk::Bytes(inner) => inner.advance(cnt),
        }
    }
}

impl<T: Buf> http_body::Body for Body<T> {
    type Data = BodyChunk<T>;
    type Error = Infallible;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        use BodyProj::*;
        match self.project() {
            Empty => Poll::Ready(None),
            Buf { inner } => match inner.take() {
                None => Poll::Ready(None),
                Some(buf) => Poll::Ready(Some(Ok(Frame::data(BodyChunk::Buf(buf))))),
            },
            Bytes { inner } => match inner.take() {
                None => Poll::Ready(None),
                Some(buf) => Poll::Ready(Some(Ok(Frame::data(BodyChunk::Bytes(buf))))),
            },
            Stream { rx } => match rx.poll_recv(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(ready) => {
                    Poll::Ready(ready.map(|bytes| Ok(Frame::data(BodyChunk::Bytes(bytes)))))
                }
            },
        }
    }

    fn is_end_stream(&self) -> bool {
        match self {
            Body::Empty => true,
            Body::Buf { inner } => inner.is_none(),
            Body::Bytes { inner } => inner.is_none(),
            Body::Stream { .. } => false,
        }
    }

    fn size_hint(&self) -> SizeHint {
        match self {
            Body::Empty => SizeHint::with_exact(0),
            Body::Buf { inner: Some(inner) } => SizeHint::with_exact(inner.remaining() as u64),
            Body::Buf { inner: None } => SizeHint::with_exact(0),
            Body::Bytes { inner: Some(inner) } => SizeHint::with_exact(inner.remaining() as u64),
            Body::Bytes { inner: None } => SizeHint::with_exact(0),
            Body::Stream { .. } => SizeHint::default(),
        }
    }
}
