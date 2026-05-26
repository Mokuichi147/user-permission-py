use std::sync::{Arc, Mutex};

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use user_permission_core::Database;

use crate::error::map_core_err;
use crate::group::PyGroupManager;
use crate::service_client::PyServiceClientManager;
use crate::token::PyTokenManager;
use crate::user::{PyUser, PyUserManager};

pub(crate) type SharedDb = Arc<Mutex<Option<Database>>>;

#[pyclass(module = "user_permission", name = "Database", unsendable)]
pub struct PyDatabase {
    target: String,
    secret: Option<String>,
    pub(crate) inner: SharedDb,
}

impl PyDatabase {
    pub(crate) fn get_db(&self) -> PyResult<Database> {
        lock_db(&self.inner)
    }
}

fn lock_db(inner: &SharedDb) -> PyResult<Database> {
    inner
        .lock()
        .map_err(|_| PyRuntimeError::new_err("internal lock poisoned"))?
        .as_ref()
        .cloned()
        .ok_or_else(|| {
            PyRuntimeError::new_err("Database is not connected. Call connect() first.")
        })
}

fn extract_str(value: &Bound<'_, PyAny>) -> PyResult<String> {
    if let Ok(s) = value.extract::<String>() {
        return Ok(s);
    }
    // pathlib.Path or os.PathLike
    value.call_method0("__fspath__")?.extract()
}

#[pymethods]
impl PyDatabase {
    #[new]
    #[pyo3(signature = (backend, *, secret=None))]
    fn new(backend: &Bound<'_, PyAny>, secret: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        let target = extract_str(backend)?;
        let secret = match secret {
            Some(s) => Some(extract_str(s)?),
            None => None,
        };
        Ok(Self {
            target,
            secret,
            inner: Arc::new(Mutex::new(None)),
        })
    }

    fn connect<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let target = self.target.clone();
        let secret = self.secret.clone();
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let db = Database::open(&target, secret.as_deref())
                .await
                .map_err(map_core_err)?;
            *inner.lock().expect("db lock poisoned") = Some(db);
            Ok(())
        })
    }

    fn close<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let db = inner.lock().expect("db lock poisoned").take();
            if let Some(db) = db {
                db.close().await.map_err(map_core_err)?;
            }
            Ok(())
        })
    }

    fn __aenter__<'py>(slf: PyRef<'py, Self>, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let target = slf.target.clone();
        let secret = slf.secret.clone();
        let inner = slf.inner.clone();
        let slf_obj: PyObject = slf.into_py(py);
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let db = Database::open(&target, secret.as_deref())
                .await
                .map_err(map_core_err)?;
            *inner.lock().expect("db lock poisoned") = Some(db);
            Ok(slf_obj)
        })
    }

    #[pyo3(signature = (*_args))]
    fn __aexit__<'py>(
        &self,
        py: Python<'py>,
        _args: &Bound<'py, pyo3::types::PyTuple>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let db = inner.lock().expect("db lock poisoned").take();
            if let Some(db) = db {
                db.close().await.map_err(map_core_err)?;
            }
            Ok(())
        })
    }

    #[getter]
    fn users(&self) -> PyUserManager {
        PyUserManager::new(self.inner.clone())
    }

    #[getter]
    fn groups(&self) -> PyGroupManager {
        PyGroupManager::new(self.inner.clone())
    }

    #[getter]
    fn service_clients(&self) -> PyServiceClientManager {
        PyServiceClientManager::new(self.inner.clone())
    }

    #[getter]
    fn token_manager(&self) -> PyResult<PyTokenManager> {
        let db = self.get_db()?;
        let tm = db.token_manager().map_err(map_core_err)?.clone();
        Ok(PyTokenManager::from_inner(tm))
    }

    fn is_local(&self) -> PyResult<bool> {
        Ok(self.get_db()?.is_local())
    }

    fn is_relay(&self) -> PyResult<bool> {
        Ok(self.get_db()?.is_relay())
    }

    fn login<'py>(
        &self,
        py: Python<'py>,
        username: String,
        password: String,
    ) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let db = lock_db(&inner)?;
            db.login(&username, &password)
                .await
                .map_err(map_core_err)
        })
    }

    fn login_client_credentials<'py>(
        &self,
        py: Python<'py>,
        client_id: String,
        client_secret: String,
    ) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let db = lock_db(&inner)?;
            db.login_client_credentials(&client_id, &client_secret)
                .await
                .map_err(map_core_err)
        })
    }

    fn verify_token_and_get_user<'py>(
        &self,
        py: Python<'py>,
        token: String,
    ) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let db = lock_db(&inner)?;
            let user = db
                .verify_token_and_get_user(&token)
                .await
                .map_err(map_core_err)?;
            Python::with_gil(|py| {
                Ok(match user {
                    Some(u) => PyUser::from(u).into_py(py),
                    None => py.None(),
                })
            })
        })
    }

    #[pyo3(signature = (username, password, display_name=""))]
    fn bootstrap_admin_if_needed<'py>(
        &self,
        py: Python<'py>,
        username: String,
        password: String,
        display_name: &str,
    ) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        let display_name = display_name.to_string();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let db = lock_db(&inner)?;
            let user = db
                .bootstrap_admin_if_needed(&username, &password, &display_name)
                .await
                .map_err(map_core_err)?;
            Python::with_gil(|py| {
                Ok(match user {
                    Some(u) => PyUser::from(u).into_py(py),
                    None => py.None(),
                })
            })
        })
    }
}
