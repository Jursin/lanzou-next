use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::lanzou::client::LanzouClient;

pub struct AppState {
    pub client: Mutex<LanzouClient>,
    /// 传输任务取消标志表（task_id -> 取消标志）
    pub cancels: Mutex<HashMap<String, Arc<std::sync::atomic::AtomicBool>>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            client: Mutex::new(LanzouClient::new()),
            cancels: Mutex::new(HashMap::new()),
        }
    }
}

impl AppState {
    /// 注册一个取消标志，返回其句柄
    pub async fn register_cancel(&self, task_id: &str) -> Arc<std::sync::atomic::AtomicBool> {
        let flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.cancels
            .lock()
            .await
            .insert(task_id.to_string(), flag.clone());
        flag
    }

    /// 触发取消
    pub async fn cancel_task(&self, task_id: &str) {
        if let Some(flag) = self.cancels.lock().await.get(task_id) {
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }

    /// 任务结束移除标志
    pub async fn finish_cancel(&self, task_id: &str) {
        self.cancels.lock().await.remove(task_id);
    }
}
