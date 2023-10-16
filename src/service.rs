use crate::{Body, ETag, Encoding};
use bytes::{Buf, Bytes, BytesMut};
use http::header::{ACCEPT_ENCODING, CONTENT_ENCODING, ETAG, IF_NONE_MATCH};
use http::{HeaderMap, HeaderValue, Method, Request, Response};
use std::sync::RwLock;
use tokio::sync::mpsc;
use tracing::{info, warn};

#[derive(Debug)]
pub struct Service<T> {
    pub headers: HeaderMap,
    encoding: Encoding,
    payload: RwLock<Payload<T>>,
}

#[derive(Debug)]
enum Payload<T> {
    Empty,
    Filled { etag: ETag, body: T },
}

impl<T> Default for Service<T> {
    fn default() -> Self {
        Self {
            headers: HeaderMap::new(),
            encoding: Encoding::Identity,
            payload: RwLock::new(Payload::Empty),
        }
    }
}

impl<T> Service<T>
where
    T: Buf + Clone + Send + 'static,
{
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_encoding(&mut self, encoding: Encoding) {
        self.encoding = encoding;
        self.headers.insert(
            CONTENT_ENCODING,
            HeaderValue::from_static(encoding.as_str()),
        );
    }

    pub fn fill(&self, body: T) {
        let etag = if body.has_remaining() {
            ETag::from_buf(body.clone())
        } else {
            ETag::empty()
        };
        *self.payload.write().unwrap() = Payload::Filled { etag, body };
    }

    pub async fn call<B>(&self, req: Request<B>) -> Response<Body<T>> {
        let head = match *req.method() {
            Method::HEAD => true,
            Method::GET => false,
            _ => {
                return method_not_allowed();
            }
        };

        let (etag, body) = {
            let buf = self.payload.read().unwrap();

            let Payload::Filled { ref etag, ref body } = *buf else {
                return no_content();
            };

            (etag.clone(), body.clone())
        };

        if let Some(if_none_match) = req.headers().get(IF_NONE_MATCH) {
            if etag.matches(if_none_match.as_bytes()) {
                return not_modified();
            }
        }

        let mut res = Response::builder().status(http::StatusCode::OK);

        for (k, v) in &self.headers {
            res = res.header(k.clone(), v.clone());
        }
        res = res.header(ETAG, etag.0);

        if head {
            return res.body(Body::Empty).unwrap();
        }

        if body.has_remaining() {
            let bytes = body.remaining();
            let encoding = self.encoding;

            let body = if let Some(accept_encoding) = req.headers().get(ACCEPT_ENCODING) {
                if encoding == Encoding::Identity || encoding.is_contained_in(accept_encoding) {
                    info!(%encoding, %bytes, "serving body");
                    Body::Buf { inner: Some(body) }
                } else {
                    res.headers_mut().unwrap().remove(CONTENT_ENCODING);
                    let spawn_decoder = match encoding {
                        Encoding::Br => spawn_br_decoder,
                        Encoding::Gzip => spawn_gzip_decoder,
                        Encoding::Deflate => spawn_deflate_decoder,
                        Encoding::Identity => unreachable!(),
                    };
                    warn!(%encoding, "decoder task is spawned");
                    Body::from(spawn_decoder(body))
                }
            } else {
                info!(%encoding, %bytes, "serving body");
                Body::Buf { inner: Some(body) }
            };

            res.body(body).unwrap()
        } else {
            res.headers_mut().unwrap().remove(CONTENT_ENCODING);
            res.body(Body::Empty).unwrap()
        }
    }
}

fn no_content<T: Buf>() -> Response<Body<T>> {
    Response::builder()
        .status(http::StatusCode::NO_CONTENT)
        .body(Body::Empty)
        .unwrap()
}

fn not_modified<T: Buf>() -> Response<Body<T>> {
    Response::builder()
        .status(http::StatusCode::NOT_MODIFIED)
        .body(Body::Empty)
        .unwrap()
}

fn method_not_allowed<T: Buf>() -> Response<Body<T>> {
    Response::builder()
        .status(http::StatusCode::METHOD_NOT_ALLOWED)
        .body(Body::from_static(b"Method not allowed"))
        .unwrap()
}

fn spawn_br_decoder(body: impl Buf + Send + 'static) -> mpsc::Receiver<Bytes> {
    spawn_decoder(brotli_decompressor::Decompressor::new(body.reader(), 512))
}

fn spawn_gzip_decoder(body: impl Buf + Send + 'static) -> mpsc::Receiver<Bytes> {
    spawn_decoder(flate2::read::GzDecoder::new(body.reader()))
}

fn spawn_deflate_decoder(body: impl Buf + Send + 'static) -> mpsc::Receiver<Bytes> {
    spawn_decoder(flate2::read::DeflateDecoder::new(body.reader()))
}

fn spawn_decoder(mut read_decoder: impl std::io::Read + Send + 'static) -> mpsc::Receiver<Bytes> {
    let (tx, rx) = mpsc::channel(1);

    tokio::task::spawn_blocking(move || loop {
        let mut buf = BytesMut::zeroed(512);
        let n = read_decoder.read(buf.as_mut()).expect("fail to read");
        if n == 0 {
            break;
        }
        tx.blocking_send(buf.split_to(n).freeze())
            .expect("fail to blocking_send");
    });

    rx
}
