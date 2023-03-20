use crate::Workspace;
use jwst::{error, info, DocStorage};
use jwst_rpc::start_client;
use jwst_storage::JwstStorage as AutoStorage;
use std::sync::Arc;
use tokio::{runtime::Runtime, sync::RwLock};

use napi::{Error, Result, Status};

#[napi]
#[derive(Clone)]
pub struct Storage {
  pub(crate) storage: Option<Arc<RwLock<AutoStorage>>>,
  pub(crate) error: Option<String>,
}

#[napi]
impl Storage {
  #[napi(constructor)]
  pub fn new(path: String) -> Self {
    let rt = Runtime::new().unwrap();

    match rt.block_on(AutoStorage::new(&format!("sqlite:{path}?mode=rwc"))) {
      Ok(pool) => Self {
        storage: Some(Arc::new(RwLock::new(pool))),
        error: None,
      },
      Err(e) => Self {
        storage: None,
        error: Some(e.to_string()),
      },
    }
  }

  #[napi]
  pub fn error(&self) -> Option<String> {
    self.error.clone()
  }

  #[napi]
  pub fn connect(&mut self, workspace_id: String, remote: String) -> Option<Workspace> {
    match self.sync(workspace_id, remote) {
      Ok(workspace) => Some(workspace),
      Err(e) => {
        error!("Failed to connect to workspace: {}", e);
        self.error = Some(e.to_string());
        None
      }
    }
  }

  #[napi]
  pub fn sync(&self, workspace_id: String, remote: String) -> Result<Workspace> {
    if let Some(storage) = &self.storage {
      let rt = Runtime::new().unwrap();

      let mut workspace = rt.block_on(async move {
        let storage = storage.read().await;

        start_client(&storage, workspace_id, remote).await
      }).map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

      let (sub, workspace) = {
        let id = workspace.id();
        let storage = self.storage.clone();
        let sub = workspace.observe(move |_, e| {
          let id = id.clone();
          if let Some(storage) = storage.clone() {
            let rt = Runtime::new().unwrap();
            info!("update: {:?}", &e.update);
            if let Err(e) = rt.block_on(async move {
              let storage = storage.write().await;
              storage.docs().write_update(id, &e.update).await
            }) {
              error!("Failed to write update to storage: {}", e);
            }
          }
        });

        (sub, workspace)
      };

      Ok(Workspace {
        workspace,
        _sub: sub,
      })
    } else {
      Err(Error::new(
        Status::GenericFailure,
        "Storage is not connected",
      ))
    }
  }
}
