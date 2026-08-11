use std::future::Future;

use gpui::Context;

use super::OneChat;

pub(super) struct TokioTaskStopped;

impl OneChat {
    pub(super) fn spawn_tokio<T>(
        &self,
        future: impl Future<Output = T> + Send + 'static,
        cx: &mut Context<Self>,
        complete: impl FnOnce(&mut Self, Result<T, TokioTaskStopped>, &mut Context<Self>) + 'static,
    ) where
        T: Send + 'static,
    {
        let (sender, receiver) = async_channel::bounded(1);
        self.services.runtime.spawn(async move {
            let _ = sender.send(future.await).await;
        });
        cx.spawn(async move |this, cx| {
            let result = receiver.recv().await.map_err(|_| TokioTaskStopped);
            let _ = this.update(cx, |this, cx| complete(this, result, cx));
        })
        .detach();
    }
}
