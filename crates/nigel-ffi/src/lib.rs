//! The Swift shell's bridge to the books: one UniFFI object, [`NigelHost`],
//! that answers `nigel://` requests from the same session-free router the
//! Tauri shell serves, and stages statements into the upload spool.
//!
//! The FFI surface is deliberately one request method rather than a typed call
//! per route, so every route's validation stays in the router and a new route
//! costs the shell nothing. The SPA is embedded in this library the way it is
//! in every build: `nigel-core`'s rust-embed over `web/dist`.
//!
//! The host knows nothing about where the folder it is given came from. In
//! the sandboxed app the shell resolves a security-scoped bookmark and starts
//! access before calling [`NigelHost::new`]; that is the shell's business.

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
    /// Open the books in `data_dir`, or — given none — the folder the CLI's
    /// settings name, so the shell and the terminal share one set of books.
    ///
    /// A named folder must already exist. The database inside it is created
    /// on first use, as the CLI creates it in a folder the user chose.
    #[uniffi::constructor]
    pub fn new(data_dir: Option<String>) -> Result<Arc<Self>, FfiError> {
        let data_dir = match data_dir {
            Some(dir) => {
                let dir = PathBuf::from(dir);
                if !dir.is_dir() {
                    return Err(FfiError::BooksFolderMissing {
                        path: dir.display().to_string(),
                    });
                }
                dir
            }
            None => nigel_core::settings::get_data_dir(),
        };
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
    pub fn open(db_path: &Path) -> Result<Self, FfiError> {
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
