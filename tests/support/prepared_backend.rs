use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

static BACKEND_ENV_LOCK: Mutex<()> = Mutex::new(());

pub struct PreparedBackendOverride {
    previous: Option<OsString>,
    _guard: MutexGuard<'static, ()>,
}

impl PreparedBackendOverride {
    pub fn try_acquire() -> Result<Self, PathBuf> {
        let guard = BACKEND_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let previous = std::env::var_os("DTS_HIGHDICOM_PYTHON");
        let configured = previous.as_ref().map(PathBuf::from).unwrap_or_else(|| {
            Path::new("generation-backends/highdicom-pydicom/.venv/bin/python").into()
        });
        if !configured.is_file() {
            return Err(configured);
        }
        let executable = if configured.is_absolute() {
            configured
        } else {
            std::env::current_dir().unwrap().join(configured)
        };
        // Preserve the venv entry-point path: canonicalizing it follows the
        // interpreter symlink and loses the prepared environment's modules.
        unsafe { std::env::set_var("DTS_HIGHDICOM_PYTHON", executable) };
        Ok(Self {
            previous,
            _guard: guard,
        })
    }

    #[allow(dead_code)]
    pub fn acquire() -> Self {
        Self::try_acquire().unwrap_or_else(|configured| {
            panic!(
                "composition qualification requires the prepared locked backend at {}",
                configured.display()
            )
        })
    }
}

impl Drop for PreparedBackendOverride {
    fn drop(&mut self) {
        match self.previous.take() {
            Some(value) => unsafe { std::env::set_var("DTS_HIGHDICOM_PYTHON", value) },
            None => unsafe { std::env::remove_var("DTS_HIGHDICOM_PYTHON") },
        }
    }
}
