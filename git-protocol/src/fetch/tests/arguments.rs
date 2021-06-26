use crate::fetch;
use bstr::ByteSlice;
use git_transport::Protocol;

fn arguments_v1(features: impl IntoIterator<Item = &'static str>) -> fetch::Arguments {
    fetch::Arguments::new(Protocol::V1, features.into_iter().map(|n| (n, None)).collect())
}

fn arguments_v2(features: impl IntoIterator<Item = &'static str>) -> fetch::Arguments {
    fetch::Arguments::new(Protocol::V2, features.into_iter().map(|n| (n, None)).collect())
}

struct Transport<T> {
    inner: T,
    stateful: bool,
}

#[cfg(feature = "blocking-client")]
mod impls {
    use crate::fetch::tests::arguments::Transport;
    use git_transport::{
        client,
        client::{Error, Identity, MessageKind, RequestWriter, SetServiceResponse, WriteMode},
        Protocol, Service,
    };

    impl<T: client::TransportWithoutIO> client::TransportWithoutIO for Transport<T> {
        fn set_identity(&mut self, identity: Identity) -> Result<(), Error> {
            self.inner.set_identity(identity)
        }

        fn request(&mut self, write_mode: WriteMode, on_into_read: MessageKind) -> Result<RequestWriter<'_>, Error> {
            self.inner.request(write_mode, on_into_read)
        }

        fn to_url(&self) -> String {
            self.inner.to_url()
        }

        fn supported_protocol_versions(&self) -> &[Protocol] {
            self.inner.supported_protocol_versions()
        }

        fn is_stateful(&self) -> bool {
            self.stateful
        }
    }

    impl<T: client::Transport> client::Transport for Transport<T> {
        fn handshake(&mut self, service: Service) -> Result<SetServiceResponse<'_>, Error> {
            self.inner.handshake(service)
        }

        fn close(&mut self) -> Result<(), Error> {
            self.inner.close()
        }
    }
}

#[cfg(feature = "async-client")]
mod impls {
    use crate::fetch::tests::arguments::Transport;
    use async_trait::async_trait;
    use git_transport::{
        client,
        client::{Error, Identity, MessageKind, RequestWriter, SetServiceResponse, WriteMode},
        Protocol, Service,
    };
    impl<T: client::TransportWithoutIO + Send> client::TransportWithoutIO for Transport<T> {
        fn set_identity(&mut self, identity: Identity) -> Result<(), Error> {
            self.inner.set_identity(identity)
        }

        fn request(&mut self, write_mode: WriteMode, on_into_read: MessageKind) -> Result<RequestWriter<'_>, Error> {
            self.inner.request(write_mode, on_into_read)
        }

        fn to_url(&self) -> String {
            self.inner.to_url()
        }

        fn supported_protocol_versions(&self) -> &[Protocol] {
            self.inner.supported_protocol_versions()
        }

        fn is_stateful(&self) -> bool {
            self.stateful
        }
    }

    #[async_trait(?Send)]
    impl<T: client::Transport + Send> client::Transport for Transport<T> {
        async fn handshake(&mut self, service: Service) -> Result<SetServiceResponse<'_>, Error> {
            self.inner.handshake(service).await
        }

        async fn close(&mut self) -> Result<(), Error> {
            self.inner.close().await
        }
    }
}

fn transport(
    out: &mut Vec<u8>,
    stateful: bool,
) -> Transport<git_transport::client::git::Connection<&'static [u8], &mut Vec<u8>>> {
    Transport {
        inner: git_transport::client::git::Connection::new(
            &[],
            out,
            Protocol::V1, // does not matter
            b"does/not/matter".as_bstr().to_owned(),
            None::<(&str, _)>,
            git_transport::client::git::ConnectMode::Process, // avoid header to be sent
        ),
        stateful,
    }
}

fn id(hex: &str) -> git_hash::ObjectId {
    git_hash::ObjectId::from_hex(hex.as_bytes()).expect("expect valid hex id")
}

mod v1 {
    use crate::fetch::tests::arguments::{arguments_v1, id, transport};
    use bstr::ByteSlice;

    #[maybe_async::test(feature = "blocking-client", async(feature = "async-client", async_std::test))]
    async fn haves_and_wants_for_clone() {
        let mut out = Vec::new();
        let mut t = transport(&mut out, true);
        let mut arguments = arguments_v1(["feature-a", "feature-b"].iter().cloned());

        arguments.want(id("7b333369de1221f9bfbbe03a3a13e9a09bc1c907"));
        arguments.want(id("ff333369de1221f9bfbbe03a3a13e9a09bc1ffff"));
        arguments.send(&mut t, true).await.expect("sending to buffer to work");
        assert_eq!(
            out.as_bstr(),
            b"0046want 7b333369de1221f9bfbbe03a3a13e9a09bc1c907 feature-a feature-b
0032want ff333369de1221f9bfbbe03a3a13e9a09bc1ffff
00000009done
"
            .as_bstr()
        );
    }

    #[maybe_async::test(feature = "blocking-client", async(feature = "async-client", async_std::test))]
    async fn haves_and_wants_for_fetch_stateless() {
        let mut out = Vec::new();
        let mut t = transport(&mut out, false);
        let mut arguments = arguments_v1(["feature-a", "shallow", "deepen-since", "deepen-not"].iter().copied());

        arguments.deepen(1);
        arguments.shallow(id("7b333369de1221f9bfbbe03a3a13e9a09bc1c9ff"));
        arguments.want(id("7b333369de1221f9bfbbe03a3a13e9a09bc1c907"));
        arguments.deepen_since(12345);
        arguments.deepen_not("refs/heads/main".into());
        arguments.have(id("0000000000000000000000000000000000000000"));
        arguments.send(&mut t, false).await.expect("sending to buffer to work");

        arguments.have(id("1111111111111111111111111111111111111111"));
        arguments.send(&mut t, true).await.expect("sending to buffer to work");
        assert_eq!(
            out.as_bstr(),
            b"005cwant 7b333369de1221f9bfbbe03a3a13e9a09bc1c907 feature-a shallow deepen-since deepen-not
0035shallow 7b333369de1221f9bfbbe03a3a13e9a09bc1c9ff
000ddeepen 1
0017deepen-since 12345
001fdeepen-not refs/heads/main
00000032have 0000000000000000000000000000000000000000
0000005cwant 7b333369de1221f9bfbbe03a3a13e9a09bc1c907 feature-a shallow deepen-since deepen-not
0035shallow 7b333369de1221f9bfbbe03a3a13e9a09bc1c9ff
000ddeepen 1
0017deepen-since 12345
001fdeepen-not refs/heads/main
00000032have 1111111111111111111111111111111111111111
0009done
"
            .as_bstr()
        );
    }

    #[maybe_async::test(feature = "blocking-client", async(feature = "async-client", async_std::test))]
    async fn haves_and_wants_for_fetch_stateful() {
        let mut out = Vec::new();
        let mut t = transport(&mut out, true);
        let mut arguments = arguments_v1(["feature-a", "shallow"].iter().copied());

        arguments.deepen(1);
        arguments.want(id("7b333369de1221f9bfbbe03a3a13e9a09bc1c907"));
        arguments.have(id("0000000000000000000000000000000000000000"));
        arguments.send(&mut t, false).await.expect("sending to buffer to work");

        arguments.have(id("1111111111111111111111111111111111111111"));
        arguments.send(&mut t, true).await.expect("sending to buffer to work");
        assert_eq!(
            out.as_bstr(),
            b"0044want 7b333369de1221f9bfbbe03a3a13e9a09bc1c907 feature-a shallow
000ddeepen 1
00000032have 0000000000000000000000000000000000000000
00000032have 1111111111111111111111111111111111111111
0009done
"
            .as_bstr()
        );
    }
}

mod v2 {
    use crate::fetch::tests::arguments::{arguments_v2, id, transport};
    use bstr::ByteSlice;

    #[maybe_async::test(feature = "blocking-client", async(feature = "async-client", async_std::test))]
    async fn haves_and_wants_for_clone_stateful() {
        let mut out = Vec::new();
        let mut t = transport(&mut out, true);
        let mut arguments = arguments_v2(["feature-a", "shallow"].iter().copied());

        arguments.deepen(1);
        arguments.deepen_relative();
        arguments.want(id("7b333369de1221f9bfbbe03a3a13e9a09bc1c907"));
        arguments.want(id("ff333369de1221f9bfbbe03a3a13e9a09bc1ffff"));
        arguments.send(&mut t, true).await.expect("sending to buffer to work");
        assert_eq!(
            out.as_bstr(),
            b"0012command=fetch
0001000ethin-pack
0010include-tag
000eofs-delta
000ddeepen 1
0014deepen-relative
0032want 7b333369de1221f9bfbbe03a3a13e9a09bc1c907
0032want ff333369de1221f9bfbbe03a3a13e9a09bc1ffff
0009done
0000"
                .as_bstr(), "we filter features/capabilities without value as these apparently sholdn't be listed (remote dies otherwise)"
        );
    }

    #[maybe_async::test(feature = "blocking-client", async(feature = "async-client", async_std::test))]
    async fn haves_and_wants_for_fetch_stateless_and_stateful() {
        for is_stateful in &[false, true] {
            let mut out = Vec::new();
            let mut t = transport(&mut out, *is_stateful);
            let mut arguments = arguments_v2(Some("shallow"));

            arguments.deepen(1);
            arguments.deepen_since(12345);
            arguments.shallow(id("7b333369de1221f9bfbbe03a3a13e9a09bc1c9ff"));
            arguments.want(id("7b333369de1221f9bfbbe03a3a13e9a09bc1c907"));
            arguments.deepen_not("refs/heads/main".into());
            arguments.have(id("0000000000000000000000000000000000000000"));
            arguments.send(&mut t, false).await.expect("sending to buffer to work");

            arguments.have(id("1111111111111111111111111111111111111111"));
            arguments.send(&mut t, true).await.expect("sending to buffer to work");
            assert_eq!(
                out.as_bstr(),
                b"0012command=fetch
0001000ethin-pack
0010include-tag
000eofs-delta
000ddeepen 1
0017deepen-since 12345
0035shallow 7b333369de1221f9bfbbe03a3a13e9a09bc1c9ff
0032want 7b333369de1221f9bfbbe03a3a13e9a09bc1c907
001fdeepen-not refs/heads/main
0032have 0000000000000000000000000000000000000000
00000012command=fetch
0001000ethin-pack
0010include-tag
000eofs-delta
000ddeepen 1
0017deepen-since 12345
0035shallow 7b333369de1221f9bfbbe03a3a13e9a09bc1c9ff
0032want 7b333369de1221f9bfbbe03a3a13e9a09bc1c907
001fdeepen-not refs/heads/main
0032have 1111111111111111111111111111111111111111
0009done
0000"
                    .as_bstr(),
                "V2 is stateless by default, so it repeats all but 'haves' in each request"
            );
        }
    }
}
