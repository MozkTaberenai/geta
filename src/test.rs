use crate::*;
use bytes::Bytes;
use http::header::{ACCEPT_ENCODING, CONTENT_ENCODING, CONTENT_TYPE, ETAG, IF_NONE_MATCH};
use http::{HeaderValue, Request, StatusCode};
use http_body_util::BodyExt;

fn test_body() -> Bytes {
    use bytes::{BufMut, BytesMut};
    let mut body = BytesMut::new();
    body.put(&include_bytes!("./lib.rs")[..]);
    body.put(&include_bytes!("./encoding.rs")[..]);
    body.put(&include_bytes!("./etag.rs")[..]);
    body.put(&include_bytes!("./body.rs")[..]);
    body.put(&include_bytes!("./service.rs")[..]);
    body.freeze()
}

#[tokio::test]
async fn get() {
    let orig_body = test_body();
    let orig_etag = ETag::from(&orig_body[..]);
    let content_type = HeaderValue::from_static("text/plain");

    let mut bufd = Service::new();
    bufd.headers.insert(CONTENT_TYPE, content_type);
    bufd.fill(orig_body.clone());

    // GET If-None-Match
    {
        let if_none_match = HeaderValue::from_maybe_shared(orig_etag.0.clone()).unwrap();

        let req = Request::get("/")
            .header(IF_NONE_MATCH, if_none_match.clone())
            .body(())
            .unwrap();

        let res = bufd.call(req).await;

        assert_eq!(res.status(), StatusCode::NOT_MODIFIED);
    }

    // HEAD request
    {
        let req = Request::head("/").body(()).unwrap();

        let res = bufd.call(req).await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers().get(ETAG).unwrap().as_bytes(),
            orig_etag.as_ref()
        );
        assert_eq!(
            res.headers().get(CONTENT_TYPE).unwrap().as_bytes(),
            b"text/plain"
        );
    }

    // GET request
    {
        let req = Request::get("/").body(()).unwrap();

        let mut res = bufd.call(req).await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers().get(ETAG).unwrap().as_bytes(),
            orig_etag.as_ref()
        );
        assert_eq!(
            res.body_mut().collect().await.unwrap().to_bytes(),
            orig_body
        );
    }
}

#[tokio::test]
async fn br() {
    let orig_body = test_body();

    let orig_body_br = {
        let mut encoder = brotli::CompressorWriter::new(vec![], 4096, 9, 22);
        std::io::copy(&mut &orig_body[..], &mut encoder).unwrap();
        Bytes::from(encoder.into_inner())
    };

    let orig_etag = ETag::from(&orig_body_br[..]);

    let mut bufd = Service::new();
    bufd.set_encoding(Encoding::Br);
    bufd.fill(orig_body_br.clone());

    // GET If-None-Match
    {
        let if_none_match = HeaderValue::from_maybe_shared(orig_etag.0.clone()).unwrap();

        let req = Request::get("/")
            .header(IF_NONE_MATCH, if_none_match.clone())
            .body(())
            .unwrap();

        let res = bufd.call(req).await;

        assert_eq!(res.status(), StatusCode::NOT_MODIFIED);
    }

    // HEAD request
    {
        let req = Request::head("/").body(()).unwrap();

        let res = bufd.call(req).await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers().get(ETAG).unwrap().as_bytes(),
            orig_etag.as_ref()
        );
        assert_eq!(
            res.headers().get(CONTENT_ENCODING).unwrap().as_bytes(),
            b"br"
        );
    }

    // GET request (no accept-encoding header)
    {
        let req = Request::get("/").body(()).unwrap();

        let mut res = bufd.call(req).await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers().get(ETAG).unwrap().as_bytes(),
            orig_etag.as_ref()
        );
        assert_eq!(
            res.headers().get(CONTENT_ENCODING).unwrap().as_bytes(),
            b"br"
        );
        assert_eq!(
            res.body_mut().collect().await.unwrap().to_bytes(),
            orig_body_br
        );
    }

    // GET request (accept-encoding: br)
    {
        let req = Request::get("/")
            .header(ACCEPT_ENCODING, "br")
            .body(())
            .unwrap();

        let mut res = bufd.call(req).await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers().get(ETAG).unwrap().as_bytes(),
            orig_etag.as_ref()
        );
        assert_eq!(
            res.headers().get(CONTENT_ENCODING).unwrap().as_bytes(),
            b"br"
        );
        assert_eq!(
            res.body_mut().collect().await.unwrap().to_bytes(),
            orig_body_br
        );
    }

    // GET request (accept-encoding: "identity")
    {
        let req = Request::get("/")
            .header(ACCEPT_ENCODING, "identity")
            .body(())
            .unwrap();

        let mut res = bufd.call(req).await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers().get(ETAG).unwrap().as_bytes(),
            orig_etag.as_ref()
        );
        assert!(res.headers().get(CONTENT_ENCODING).is_none());
        assert_eq!(
            res.body_mut().collect().await.unwrap().to_bytes(),
            orig_body
        );
    }
}

#[tokio::test]
async fn gzip() {
    let orig_body = test_body();

    let orig_body_gzip = {
        let mut encoder = flate2::write::GzEncoder::new(vec![], flate2::Compression::best());
        std::io::copy(&mut &orig_body[..], &mut encoder).unwrap();
        Bytes::from(encoder.finish().unwrap())
    };

    let orig_etag = ETag::from(&orig_body_gzip[..]);

    let mut bufd = Service::new();
    bufd.set_encoding(Encoding::Gzip);
    bufd.fill(orig_body_gzip.clone());

    // GET If-None-Match
    {
        let if_none_match = HeaderValue::from_maybe_shared(orig_etag.0.clone()).unwrap();

        let req = Request::get("/")
            .header(IF_NONE_MATCH, if_none_match.clone())
            .body(())
            .unwrap();

        let res = bufd.call(req).await;

        assert_eq!(res.status(), StatusCode::NOT_MODIFIED);
    }

    // HEAD request
    {
        let req = Request::head("/").body(()).unwrap();

        let res = bufd.call(req).await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers().get(ETAG).unwrap().as_bytes(),
            orig_etag.as_ref()
        );
        assert_eq!(
            res.headers().get(CONTENT_ENCODING).unwrap().as_bytes(),
            b"gzip"
        );
    }

    // GET request (no accept-encoding header)
    {
        let req = Request::get("/").body(()).unwrap();

        let mut res = bufd.call(req).await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers().get(ETAG).unwrap().as_bytes(),
            orig_etag.as_ref()
        );
        assert_eq!(
            res.headers().get(CONTENT_ENCODING).unwrap().as_bytes(),
            b"gzip"
        );
        assert_eq!(
            res.body_mut().collect().await.unwrap().to_bytes(),
            orig_body_gzip
        );
    }

    // GET request (accept-encoding: gzip)
    {
        let req = Request::get("/")
            .header(ACCEPT_ENCODING, "gzip")
            .body(())
            .unwrap();

        let mut res = bufd.call(req).await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers().get(ETAG).unwrap().as_bytes(),
            orig_etag.as_ref()
        );
        assert_eq!(
            res.headers().get(CONTENT_ENCODING).unwrap().as_bytes(),
            b"gzip"
        );
        assert_eq!(
            res.body_mut().collect().await.unwrap().to_bytes(),
            orig_body_gzip
        );
    }

    // GET request (accept-encoding: "identity")
    {
        let req = Request::get("/")
            .header(ACCEPT_ENCODING, "identity")
            .body(())
            .unwrap();

        let mut res = bufd.call(req).await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers().get(ETAG).unwrap().as_bytes(),
            orig_etag.as_ref()
        );
        assert!(res.headers().get(CONTENT_ENCODING).is_none());
        assert_eq!(
            res.body_mut().collect().await.unwrap().to_bytes(),
            orig_body
        );
    }
}

#[tokio::test]
async fn deflate() {
    let orig_body = test_body();

    let orig_body_deflate = {
        let mut encoder = flate2::write::DeflateEncoder::new(vec![], flate2::Compression::best());
        std::io::copy(&mut &orig_body[..], &mut encoder).unwrap();
        Bytes::from(encoder.finish().unwrap())
    };

    let orig_etag = ETag::from(&orig_body_deflate[..]);

    let mut bufd = Service::new();
    bufd.set_encoding(Encoding::Deflate);
    bufd.fill(orig_body_deflate.clone());

    // GET If-None-Match
    {
        let if_none_match = HeaderValue::from_maybe_shared(orig_etag.0.clone()).unwrap();

        let req = Request::get("/")
            .header(IF_NONE_MATCH, if_none_match.clone())
            .body(())
            .unwrap();

        let res = bufd.call(req).await;

        assert_eq!(res.status(), StatusCode::NOT_MODIFIED);
    }

    // HEAD request
    {
        let req = Request::head("/").body(()).unwrap();

        let res = bufd.call(req).await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers().get(ETAG).unwrap().as_bytes(),
            orig_etag.as_ref()
        );
        assert_eq!(
            res.headers().get(CONTENT_ENCODING).unwrap().as_bytes(),
            b"deflate"
        );
    }

    // GET request (no accept-encoding header)
    {
        let req = Request::get("/").body(()).unwrap();

        let mut res = bufd.call(req).await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers().get(ETAG).unwrap().as_bytes(),
            orig_etag.as_ref()
        );
        assert_eq!(
            res.headers().get(CONTENT_ENCODING).unwrap().as_bytes(),
            b"deflate"
        );
        assert_eq!(
            res.body_mut().collect().await.unwrap().to_bytes(),
            orig_body_deflate
        );
    }

    // GET request (accept-encoding: deflate)
    {
        let req = Request::get("/")
            .header(ACCEPT_ENCODING, "deflate")
            .body(())
            .unwrap();

        let mut res = bufd.call(req).await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers().get(ETAG).unwrap().as_bytes(),
            orig_etag.as_ref()
        );
        assert_eq!(
            res.headers().get(CONTENT_ENCODING).unwrap().as_bytes(),
            b"deflate"
        );
        assert_eq!(
            res.body_mut().collect().await.unwrap().to_bytes(),
            orig_body_deflate
        );
    }

    // GET request (accept-encoding: "identity")
    {
        let req = Request::get("/")
            .header(ACCEPT_ENCODING, "identity")
            .body(())
            .unwrap();

        let mut res = bufd.call(req).await;

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers().get(ETAG).unwrap().as_bytes(),
            orig_etag.as_ref()
        );
        assert!(res.headers().get(CONTENT_ENCODING).is_none());
        assert_eq!(
            res.body_mut().collect().await.unwrap().to_bytes(),
            orig_body
        );
    }
}
