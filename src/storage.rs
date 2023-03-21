use crate::Workspace;
use jwst::{error, info, BlobStorage, DocStorage};
use jwst_rpc::start_client;
use jwst_storage::JwstStorage as AutoStorage;
use std::sync::Arc;
use tokio::{runtime::Runtime, sync::RwLock};
use futures::prelude::*;
use napi::bindgen_prelude::*;
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
  pub async fn get_blob(&self, workspace_id: Option<String>, id: String) -> Result<Buffer> {
    if let Some(storage) = &self.storage {
      let storage_handle = storage.read().await;
      let blobs = storage_handle.blobs();
      if let Ok(mut file_stream) = blobs.get_blob(workspace_id.clone(), id.clone()).await {
        // Read all of the chunks into a vector.
        let mut stream_contents = Vec::new();
        let mut error_message = "".to_string();
        while let Some(chunk) = file_stream.next().await {
          match chunk {
            Ok(chunk_bytes) => stream_contents.extend_from_slice(&chunk_bytes),
            Err(err) => {
              error_message = format!(
                "Failed to read blob file {}/{} from stream, error: {}",
                workspace_id.clone().unwrap_or_default().to_string(),
                id,
                err
              );
            }
          }
        }
        if error_message.len() > 0 {
          return Err(Error::new(Status::GenericFailure, error_message));
        }
        return Ok(stream_contents.into());
      } else {
        return Err(Error::new(
          Status::GenericFailure,
          "Storage is not connected",
        ));
      }
    } else {
      return Err(Error::new(
        Status::GenericFailure,
        "Storage is not connected",
      ));
    }
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

      let mut workspace = rt
        .block_on(async move {
          let storage = storage.read().await;

          start_client(&storage, workspace_id, remote).await
        })
        .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;

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
