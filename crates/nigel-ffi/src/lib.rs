//! The Swift shell's bridge to the books: one UniFFI object, [`NigelHost`],
//! that answers `nigel://` requests from the same session-free router the
//! Tauri shell serves, and stages statements into the upload spool.
//!
//! The FFI surface is deliberately one request method rather than a typed call
//! per route, so every route's validation stays in the router and a new route
//! costs the shell nothing. The SPA is embedded in this library the way it is
//! in every build: `nigel-core`'s rust-embed over `web/dist`.
//!
//! The host knows nothing about where the folders it is given came from. In
//! the sandboxed app `$HOME` is the app container, so the shell owns both the
//! books folder (a security-scoped bookmark it starts access to) and the
//! config folder, and passes them to [`NigelHost::new`].

use std::path::{Path, PathBuf};
use std::sync::Arc;

use nigel_core::server::state::AppState;
use nigel_core::server::{auth, build_desktop_router, scheme, uploads};

pub mod transport;

pub use transport::{SchemeHeader, SchemeRequest, SchemeResponse};

uniffi::setup_scaffolding!();

/// The database's name inside a books folder — the name the CLI uses.
const DATABASE_FILE: &str = "nigel.db";

/// Why a call the shell made could not be answered.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum FfiError {
    /// The books folder the shell named is not a directory on this machine.
    /// The host never creates one: a missing folder is the shell's cue to ask
    /// the user again, not a reason to start empty books somewhere new.
    #[error("The books folder {path} isn't there.")]
    BooksFolderMissing { path: String },
    /// The config folder the shell named is not a directory on this machine.
    #[error("The config folder {path} isn't there.")]
    ConfigFolderMissing { path: String },
    /// This process already resolves its configuration from another folder;
    /// the config folder is fixed for the life of the process.
    #[error("Configuration already lives in {current}, not {requested}.")]
    ConfigFolderConflict { current: String, requested: String },
    /// The async runtime that serves requests could not start.
    #[error("Couldn't start the request runtime: {message}")]
    Runtime { message: String },
    /// A statement could not be staged; the message is the one a browser
    /// upload of the same file would show.
    #[error("{message}")]
    Stage { message: String },
}

/// A statement spooled from disk, field for field what
/// `POST /api/imports/upload` answers, plus the path it came from.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct StagedUpload {
    pub upload_id: String,
    pub filename: String,
    pub size: u64,
    pub path: String,
}

impl From<uploads::StagedUpload> for StagedUpload {
    fn from(staged: uploads::StagedUpload) -> Self {
        Self {
            upload_id: staged.upload_id,
            filename: staged.filename,
            size: staged.size,
            path: staged.path,
        }
    }
}

/// The books, served in process over the custom scheme.
#[derive(uniffi::Object)]
pub struct NigelHost {
    state: AppState,
    router: axum::Router,
    /// `Option` only so `Drop` can shut it down without blocking: dropping a
    /// runtime from inside an async context panics, and the foreign side may
    /// release the host from anywhere.
    runtime: Option<tokio::runtime::Runtime>,
}

#[uniffi::export]
impl NigelHost {
    /// Open the books in `data_dir`, with this process's configuration
    /// (`settings.json`, the invoicing and documents config) under
    /// `config_dir`.
    ///
    /// Both are explicit because the shell owns them: in the sandbox the home
    /// directory is the app container, so nothing here may derive a location
    /// from it. Both folders must already exist — the host never creates a
    /// books folder, so a moved or deleted one is an error the shell answers
    /// by asking again rather than empty books somewhere new. The database
    /// inside an existing folder is created on first use, as the CLI creates
    /// it in a folder the user chose. The config folder is fixed for the life
    /// of the process; a second host naming a different one is refused.
    #[uniffi::constructor]
    pub fn new(data_dir: String, config_dir: String) -> Result<Arc<Self>, FfiError> {
        let data_dir = PathBuf::from(data_dir);
        if !data_dir.is_dir() {
            return Err(FfiError::BooksFolderMissing {
                path: data_dir.display().to_string(),
            });
        }
        let config_dir = PathBuf::from(config_dir);
        if !config_dir.is_dir() {
            return Err(FfiError::ConfigFolderMissing {
                path: config_dir.display().to_string(),
            });
        }
        nigel_core::settings::set_config_dir(config_dir.clone()).map_err(|current| {
            FfiError::ConfigFolderConflict {
                current: current.display().to_string(),
                requested: config_dir.display().to_string(),
            }
        })?;
        Self::open(&data_dir.join(DATABASE_FILE)).map(Arc::new)
    }

    /// Answer one scheme request: status, headers and the whole body.
    ///
    /// The router runs on the host's own multi-threaded tokio runtime, so the
    /// foreign side may await this from any executor.
    pub async fn request(&self, request: SchemeRequest) -> SchemeResponse {
        let work = transport::answer(self.router.clone(), request);
        match self.runtime().spawn(work).await {
            Ok(response) => response,
            Err(e) => SchemeResponse::plain(500, format!("the request did not finish: {e}")),
        }
    }

    /// Spool a statement already on disk into the folder a browser upload
    /// lands in, and answer with the `uploadId` the import routes take.
    ///
    /// Blocking: it reads and writes the whole file, so the shell calls it
    /// off the main thread.
    pub fn stage_file(&self, path: String) -> Result<StagedUpload, FfiError> {
        uploads::stage_file(
            Path::new(&path),
            &uploads::uploads_dir(&self.state.db_path()),
        )
        .map(StagedUpload::from)
        .map_err(|message| FfiError::Stage { message })
    }

    /// The folder the open books live in. Follows a switch made through
    /// Settings, since the router can repoint the database at runtime.
    pub fn data_dir(&self) -> String {
        self.state.data_dir().display().to_string()
    }
}

impl NigelHost {
    /// The host over a database at an explicit path.
    fn open(db_path: &Path) -> Result<Self, FfiError> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("nigel-host")
            .build()
            .map_err(|e| FfiError::Runtime {
                message: e.to_string(),
            })?;
        let state = AppState::new(db_path.to_path_buf(), auth::generate_token());
        let router = {
            let _entered = runtime.enter();
            build_desktop_router(state.clone(), scheme::trusted_origins())
        };
        Ok(Self {
            state,
            router,
            runtime: Some(runtime),
        })
    }

    fn runtime(&self) -> &tokio::runtime::Runtime {
        self.runtime
            .as_ref()
            .expect("the runtime is only taken on drop")
    }
}

impl Drop for NigelHost {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}
