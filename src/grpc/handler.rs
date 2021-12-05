use crate::grpc::controller::Controller;
use crate::state::global::GlobalState;
use futures::Future;
use orchestra::rpc::rollup::*;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, RwLock};
use tonic::{Request, Response, Status};

type ControllerAction = Box<dyn FnOnce(Arc<RwLock<Controller>>) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send>;

pub struct Handler {
    close_sender: Option<oneshot::Sender<()>>,
    controller: Arc<RwLock<Controller>>,
    task_dispatcher: mpsc::Sender<ControllerAction>,
}

impl Handler {
    pub async fn new(state: Arc<std::sync::RwLock<GlobalState>>) -> Self {
        let controller = Arc::new(RwLock::new(Controller::new(state).await));
        let controller_dispatch = Arc::clone(&controller);

        let (task_dispatcher, mut task_receiver) = mpsc::channel(16);
        let (close_sender, mut close_receiver) = oneshot::channel();

        let result = Self {
            close_sender: Some(close_sender),
            controller,
            task_dispatcher,
        };

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    task = task_receiver.recv() => {
                        let task = task.expect("Server scheduler has unexpected exit");
                        task(controller_dispatch.clone()).await;
                    }
                    _ = &mut close_receiver => {
                        log::info!("Server scheduler is notified to close");
                        task_receiver.close();
                        break;
                    }
                }
            }

            while let Some(task) = task_receiver.recv().await {
                task(Arc::clone(&controller_dispatch)).await;
            }
        });

        result
    }

    pub fn on_leave(&mut self) -> ServerLeave {
        ServerLeave(
            self.close_sender.take().expect("Do not call twice with on_leave"),
            self.task_dispatcher.clone(),
        )
    }
}

pub struct ServerLeave(oneshot::Sender<()>, mpsc::Sender<ControllerAction>);

impl ServerLeave {
    pub async fn leave(self) {
        self.0.send(()).unwrap();
        self.1.closed().await;
    }
}

#[tonic::async_trait]
impl rollup_state_server::RollupState for Handler {
    async fn l2_blocks_query(&self, request: Request<L2BlocksQueryRequest>) -> Result<Response<L2BlocksQueryResponse>, Status> {
        let controller = self.controller.read().await;
        Ok(Response::new(controller.l2_blocks_query(request.into_inner()).await?))
    }

    async fn l2_block_query(&self, request: Request<L2BlockQueryRequest>) -> Result<Response<L2BlockQueryResponse>, Status> {
        let controller = self.controller.read().await;
        Ok(Response::new(controller.l2_block_query(request.into_inner()).await?))
    }

    async fn token_balance_query(&self, request: Request<TokenBalanceQueryRequest>) -> Result<Response<TokenBalanceQueryResponse>, Status> {
        let controller = self.controller.read().await;
        Ok(Response::new(controller.token_balance_query(request.into_inner())?))
    }

    async fn user_info_query(&self, request: Request<UserInfoQueryRequest>) -> Result<Response<UserInfoQueryResponse>, Status> {
        let controller = self.controller.read().await;
        Ok(Response::new(controller.user_info_query(request.into_inner()).await?))
    }

    async fn register_user(&self, request: Request<RegisterUserRequest>) -> Result<Response<RegisterUserResponse>, Status> {
        let ControllerDispatch(action, receiver) =
            ControllerDispatch::new(move |ctrl: &mut Controller| Box::pin(async move { ctrl.register_user(true, request.into_inner()) }));
        self.task_dispatcher.send(action).await.map_err(map_dispatch_err)?;
        map_dispatch_result(receiver.await)
    }
}

struct ControllerDispatch<OT>(ControllerAction, oneshot::Receiver<OT>);

impl<OT: 'static + Send> ControllerDispatch<OT> {
    fn new<T>(f: T) -> Self
    where
        T: for<'c> FnOnce(&'c mut Controller) -> Pin<Box<dyn futures::Future<Output = OT> + Send + 'c>>,
        T: Send + 'static,
    {
        let (tx, rx) = oneshot::channel();

        ControllerDispatch(
            Box::new(
                move |ctrl: Arc<RwLock<Controller>>| -> Pin<Box<dyn futures::Future<Output = ()> + Send + 'static>> {
                    Box::pin(async move {
                        let mut wg = ctrl.write().await;
                        if let Err(_t) = tx.send(f(&mut wg).await) {
                            log::error!("Controller action can not be return");
                        }
                    })
                },
            ),
            rx,
        )
    }
}

fn map_dispatch_err<T: 'static>(_: mpsc::error::SendError<T>) -> Status {
    Status::unknown("Server temporary unavaliable")
}

fn map_dispatch_result<OT: 'static>(result: Result<Result<OT, Status>, oneshot::error::RecvError>) -> Result<Response<OT>, Status> {
    match result {
        Ok(result) => result.map(Response::new),
        Err(_) => Err(Status::unknown("Dispatch ret unreach")),
    }
}
