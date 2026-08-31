use std::ffi::OsString;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

static BACKEND_ENV_LOCK: Mutex<()> = Mutex::new(());

pub struct PreparedBackendOverride {
    previous: Option<OsString>,
    _guard: MutexGuard<'static, ()>,
}

impl PreparedBackendOverride {
    pub fn acquire() -> Self {
        let guard = BACKEND_ENV_LOCK.lock().unwrap();
        let relative = Path::new("generation-backends/highdicom-pydicom/.venv/bin/python");
        assert!(
            relative.is_file(),
            "composition qualification requires the prepared locked backend"
        );
        let executable = std::env::current_dir().unwrap().join(relative);
        let previous = std::env::var_os("DTS_HIGHDICOM_PYTHON");
        // Preserve the venv entry-point path: canonicalizing it follows the
        // interpreter symlink and loses the prepared environment's modules.
        unsafe { std::env::set_var("DTS_HIGHDICOM_PYTHON", executable) };
        Self {
            previous,
            _guard: guard,
        }
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
