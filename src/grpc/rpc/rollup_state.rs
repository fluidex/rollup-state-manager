#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, ::prost::Message)]
pub struct TokenBalanceQueryRequest {
    #[prost(uint32, tag = "1")]
    pub account_id: u32,
    #[prost(uint32, tag = "2")]
    pub token_id: u32,
    #[prost(string, tag = "3")]
    pub token_address: ::prost::alloc::string::String,
    #[prost(string, tag = "4")]
    pub token_name: ::prost::alloc::string::String,
}
#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, ::prost::Message)]
pub struct TokenBalanceQueryResponse {
    #[prost(string, tag = "1")]
    pub balance: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub balance_raw: ::prost::alloc::string::String,
    #[prost(uint32, tag = "3")]
    pub precision: u32,
}
#[doc = r" Generated client implementations."]
pub mod rollup_state_client {
    #![allow(unused_variables, dead_code, missing_docs)]
    use tonic::codegen::*;
    pub struct RollupStateClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl RollupStateClient<tonic::transport::Channel> {
        #[doc = r" Attempt to create a new client by connecting to a given endpoint."]
        pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: std::convert::TryInto<tonic::transport::Endpoint>,
            D::Error: Into<StdError>,
        {
            let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
            Ok(Self::new(conn))
        }
    }
    impl<T> RollupStateClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::BoxBody>,
        T::ResponseBody: Body + HttpBody + Send + 'static,
        T::Error: Into<StdError>,
        <T::ResponseBody as HttpBody>::Error: Into<StdError> + Send,
    {
        pub fn new(inner: T) -> Self {
            let inner = tonic::client::Grpc::new(inner);
            Self { inner }
        }
        pub fn with_interceptor(inner: T, interceptor: impl Into<tonic::Interceptor>) -> Self {
            let inner = tonic::client::Grpc::with_interceptor(inner, interceptor);
            Self { inner }
        }
        pub async fn token_balance_query(
            &mut self,
            request: impl tonic::IntoRequest<super::TokenBalanceQueryRequest>,
        ) -> Result<tonic::Response<super::TokenBalanceQueryResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| tonic::Status::new(tonic::Code::Unknown, format!("Service was not ready: {}", e.into())))?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/rollup_state.RollupState/TokenBalanceQuery");
            self.inner.unary(request.into_request(), path, codec).await
        }
    }
    impl<T: Clone> Clone for RollupStateClient<T> {
        fn clone(&self) -> Self {
            Self { inner: self.inner.clone() }
        }
    }
    impl<T> std::fmt::Debug for RollupStateClient<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "RollupStateClient {{ ... }}")
        }
    }
}
#[doc = r" Generated server implementations."]
pub mod rollup_state_server {
    #![allow(unused_variables, dead_code, missing_docs)]
    use tonic::codegen::*;
    #[doc = "Generated trait containing gRPC methods that should be implemented for use with RollupStateServer."]
    #[async_trait]
    pub trait RollupState: Send + Sync + 'static {
        async fn token_balance_query(
            &self,
            request: tonic::Request<super::TokenBalanceQueryRequest>,
        ) -> Result<tonic::Response<super::TokenBalanceQueryResponse>, tonic::Status>;
    }
    #[derive(Debug)]
    pub struct RollupStateServer<T: RollupState> {
        inner: _Inner<T>,
    }
    struct _Inner<T>(Arc<T>, Option<tonic::Interceptor>);
    impl<T: RollupState> RollupStateServer<T> {
        pub fn new(inner: T) -> Self {
            let inner = Arc::new(inner);
            let inner = _Inner(inner, None);
            Self { inner }
        }
        pub fn with_interceptor(inner: T, interceptor: impl Into<tonic::Interceptor>) -> Self {
            let inner = Arc::new(inner);
            let inner = _Inner(inner, Some(interceptor.into()));
            Self { inner }
        }
    }
    impl<T, B> Service<http::Request<B>> for RollupStateServer<T>
    where
        T: RollupState,
        B: HttpBody + Send + Sync + 'static,
        B::Error: Into<StdError> + Send + 'static,
    {
        type Response = http::Response<tonic::body::BoxBody>;
        type Error = Never;
        type Future = BoxFuture<Self::Response, Self::Error>;
        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, req: http::Request<B>) -> Self::Future {
            let inner = self.inner.clone();
            match req.uri().path() {
                "/rollup_state.RollupState/TokenBalanceQuery" => {
                    #[allow(non_camel_case_types)]
                    struct TokenBalanceQuerySvc<T: RollupState>(pub Arc<T>);
                    impl<T: RollupState> tonic::server::UnaryService<super::TokenBalanceQueryRequest> for TokenBalanceQuerySvc<T> {
                        type Response = super::TokenBalanceQueryResponse;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(&mut self, request: tonic::Request<super::TokenBalanceQueryRequest>) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).token_balance_query(request).await };
                            Box::pin(fut)
                        }
                    }
                    let inner = self.inner.clone();
                    let fut = async move {
                        let interceptor = inner.1.clone();
                        let inner = inner.0;
                        let method = TokenBalanceQuerySvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = if let Some(interceptor) = interceptor {
                            tonic::server::Grpc::with_interceptor(codec, interceptor)
                        } else {
                            tonic::server::Grpc::new(codec)
                        };
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                _ => Box::pin(async move {
                    Ok(http::Response::builder()
                        .status(200)
                        .header("grpc-status", "12")
                        .header("content-type", "application/grpc")
                        .body(tonic::body::BoxBody::empty())
                        .unwrap())
                }),
            }
        }
    }
    impl<T: RollupState> Clone for RollupStateServer<T> {
        fn clone(&self) -> Self {
            let inner = self.inner.clone();
            Self { inner }
        }
    }
    impl<T: RollupState> Clone for _Inner<T> {
        fn clone(&self) -> Self {
            Self(self.0.clone(), self.1.clone())
        }
    }
    impl<T: std::fmt::Debug> std::fmt::Debug for _Inner<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self.0)
        }
    }
    impl<T: RollupState> tonic::transport::NamedService for RollupStateServer<T> {
        const NAME: &'static str = "rollup_state.RollupState";
    }
}
