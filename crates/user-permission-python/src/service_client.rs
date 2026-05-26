use std::time::Duration;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use user_permission_core::ServiceClient;

use crate::database::SharedDb;
use crate::error::map_core_err;

#[pyclass(module = "user_permission", name = "ServiceClient", get_all)]
#[derive(Clone)]
pub struct PyServiceClient {
    pub id: i64,
    pub client_id: String,
    pub name: String,
    pub scopes: Vec<String>,
    pub is_active: bool,
    pub expires_at: Option<String>,
    pub created_at: String,
    pub last_used_at: Option<String>,
}

impl From<ServiceClient> for PyServiceClient {
    fn from(c: ServiceClient) -> Self {
        Self {
            id: c.id,
            client_id: c.client_id,
            name: c.name,
            scopes: c.scopes,
            is_active: c.is_active,
            expires_at: c.expires_at,
            created_at: c.created_at,
            last_used_at: c.last_used_at,
        }
    }
}

#[pymethods]
impl PyServiceClient {
    fn __repr__(&self) -> String {
        format!(
            "ServiceClient(id={}, client_id={:?}, name={:?}, scopes={:?}, is_active={})",
            self.id, self.client_id, self.name, self.scopes, self.is_active
        )
    }
}

/// Reject any scope not part of the read-only scope set (mirrors
/// `user_permission_core::validate_scopes`). Raises `ValueError` on an
/// unknown scope.
#[pyfunction]
pub fn validate_scopes(scopes: Vec<String>) -> PyResult<()> {
    user_permission_core::validate_scopes(&scopes).map_err(map_core_err)
}

#[pyclass(module = "user_permission", name = "ServiceClientManager", unsendable)]
pub struct PyServiceClientManager {
    db: SharedDb,
}

impl PyServiceClientManager {
    pub(crate) fn new(db: SharedDb) -> Self {
        Self { db }
    }
}

fn get_db(db: &SharedDb) -> PyResult<user_permission_core::Database> {
    db.lock()
        .expect("db lock poisoned")
        .as_ref()
        .cloned()
        .ok_or_else(|| {
            PyRuntimeError::new_err("Database is not connected. Call connect() first.")
        })
}

fn pydelta_to_duration(delta: Option<&Bound<'_, PyAny>>) -> PyResult<Duration> {
    match delta {
        Some(d) => {
            let total_seconds: f64 = d.call_method0("total_seconds")?.extract()?;
            if total_seconds <= 0.0 {
                Ok(Duration::from_secs(0))
            } else {
                Ok(Duration::from_secs_f64(total_seconds))
            }
        }
        None => Ok(Duration::from_secs(3600)),
    }
}

#[pymethods]
impl PyServiceClientManager {
    #[pyo3(signature = (name, scopes, expires_at=None))]
    fn create<'py>(
        &self,
        py: Python<'py>,
        name: String,
        scopes: Vec<String>,
        expires_at: Option<String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let db = get_db(&self.db)?;
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let (client, secret) = db
                .service_clients()
                .create(&name, &scopes, expires_at.as_deref())
                .await
                .map_err(map_core_err)?;
            Python::with_gil(|py| {
                let result: PyObject = (PyServiceClient::from(client), secret).into_py(py);
                Ok(result)
            })
        })
    }

    fn list<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let db = get_db(&self.db)?;
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let clients = db.service_clients().list().await.map_err(map_core_err)?;
            Python::with_gil(|py| {
                Ok(clients
                    .into_iter()
                    .map(PyServiceClient::from)
                    .collect::<Vec<_>>()
                    .into_py(py))
            })
        })
    }

    fn get_by_client_id<'py>(
        &self,
        py: Python<'py>,
        client_id: String,
    ) -> PyResult<Bound<'py, PyAny>> {
        let db = get_db(&self.db)?;
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let client = db
                .service_clients()
                .get_by_client_id(&client_id)
                .await
                .map_err(map_core_err)?;
            Python::with_gil(|py| {
                Ok(match client {
                    Some(c) => PyServiceClient::from(c).into_py(py),
                    None => py.None(),
                })
            })
        })
    }

    fn delete<'py>(&self, py: Python<'py>, id: i64) -> PyResult<Bound<'py, PyAny>> {
        let db = get_db(&self.db)?;
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            db.service_clients()
                .delete(id)
                .await
                .map_err(map_core_err)
        })
    }

    fn rotate_secret<'py>(&self, py: Python<'py>, id: i64) -> PyResult<Bound<'py, PyAny>> {
        let db = get_db(&self.db)?;
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let secret = db
                .service_clients()
                .rotate_secret(id)
                .await
                .map_err(map_core_err)?;
            Python::with_gil(|py| {
                Ok(match secret {
                    Some(s) => s.into_py(py),
                    None => py.None(),
                })
            })
        })
    }

    #[pyo3(signature = (client_id, secret, expires_delta=None))]
    fn authenticate<'py>(
        &self,
        py: Python<'py>,
        client_id: String,
        secret: String,
        expires_delta: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let db = get_db(&self.db)?;
        let duration = pydelta_to_duration(expires_delta)?;
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let token = db
                .service_clients()
                .authenticate(&client_id, &secret, duration)
                .await
                .map_err(map_core_err)?;
            Python::with_gil(|py| {
                Ok(match token {
                    Some(t) => t.into_py(py),
                    None => py.None(),
                })
            })
        })
    }
}
