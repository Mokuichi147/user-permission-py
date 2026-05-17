use std::time::Duration;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use user_permission_core::{User, UserUpdate};

use crate::database::SharedDb;
use crate::error::map_core_err;

#[pyclass(module = "user_permission", name = "User", get_all)]
#[derive(Clone)]
pub struct PyUser {
    pub id: i64,
    pub username: String,
    pub display_name: String,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<User> for PyUser {
    fn from(u: User) -> Self {
        Self {
            id: u.id,
            username: u.username,
            display_name: u.display_name,
            is_active: u.is_active,
            created_at: u.created_at,
            updated_at: u.updated_at,
        }
    }
}

#[pymethods]
impl PyUser {
    fn __repr__(&self) -> String {
        format!(
            "User(id={}, username={:?}, display_name={:?}, is_active={})",
            self.id, self.username, self.display_name, self.is_active
        )
    }
}

#[pyclass(module = "user_permission", name = "UserManager", unsendable)]
pub struct PyUserManager {
    db: SharedDb,
}

impl PyUserManager {
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
impl PyUserManager {
    #[pyo3(signature = (username, password, display_name="", *, token=None))]
    fn create<'py>(
        &self,
        py: Python<'py>,
        username: String,
        password: String,
        display_name: &str,
        token: Option<String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let db = get_db(&self.db)?;
        let display_name = display_name.to_string();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let user = db
                .users()
                .create(&username, &password, &display_name, token.as_deref())
                .await
                .map_err(map_core_err)?;
            Python::with_gil(|py| Ok(PyUser::from(user).into_py(py)))
        })
    }

    #[pyo3(signature = (user_id, *, token=None))]
    fn get_by_id<'py>(
        &self,
        py: Python<'py>,
        user_id: i64,
        token: Option<String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let db = get_db(&self.db)?;
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let user = db
                .users()
                .get_by_id(user_id, token.as_deref())
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

    #[pyo3(signature = (username, *, token=None))]
    fn get_by_username<'py>(
        &self,
        py: Python<'py>,
        username: String,
        token: Option<String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let db = get_db(&self.db)?;
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let user = db
                .users()
                .get_by_username(&username, token.as_deref())
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

    #[pyo3(signature = (*, token=None))]
    fn list_all<'py>(
        &self,
        py: Python<'py>,
        token: Option<String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let db = get_db(&self.db)?;
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let users = db
                .users()
                .list_all(token.as_deref())
                .await
                .map_err(map_core_err)?;
            Python::with_gil(|py| {
                Ok(users
                    .into_iter()
                    .map(PyUser::from)
                    .collect::<Vec<_>>()
                    .into_py(py))
            })
        })
    }

    #[pyo3(signature = (user_id, *, username=None, password=None, display_name=None, is_active=None, token=None))]
    fn update<'py>(
        &self,
        py: Python<'py>,
        user_id: i64,
        username: Option<String>,
        password: Option<String>,
        display_name: Option<String>,
        is_active: Option<bool>,
        token: Option<String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let db = get_db(&self.db)?;
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let updated = db
                .users()
                .update(
                    user_id,
                    UserUpdate {
                        username,
                        password,
                        display_name,
                        is_active,
                    },
                    token.as_deref(),
                )
                .await
                .map_err(map_core_err)?;
            Python::with_gil(|py| {
                Ok(match updated {
                    Some(u) => PyUser::from(u).into_py(py),
                    None => py.None(),
                })
            })
        })
    }

    #[pyo3(signature = (user_id, *, token=None))]
    fn delete<'py>(
        &self,
        py: Python<'py>,
        user_id: i64,
        token: Option<String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let db = get_db(&self.db)?;
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            db.users()
                .delete(user_id, token.as_deref())
                .await
                .map_err(map_core_err)
        })
    }

    #[pyo3(signature = (user_id, *, token=None))]
    fn is_admin<'py>(
        &self,
        py: Python<'py>,
        user_id: i64,
        token: Option<String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let db = get_db(&self.db)?;
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            db.users()
                .is_admin(user_id, token.as_deref())
                .await
                .map_err(map_core_err)
        })
    }

    #[pyo3(signature = (user_id, is_admin, *, token=None))]
    fn set_admin<'py>(
        &self,
        py: Python<'py>,
        user_id: i64,
        is_admin: bool,
        token: Option<String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let db = get_db(&self.db)?;
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            db.users()
                .set_admin(user_id, is_admin, token.as_deref())
                .await
                .map_err(map_core_err)
        })
    }

    #[pyo3(signature = (username, password, expires_delta=None))]
    fn authenticate<'py>(
        &self,
        py: Python<'py>,
        username: String,
        password: String,
        expires_delta: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let db = get_db(&self.db)?;
        let duration = pydelta_to_duration(expires_delta)?;
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let token = db
                .users()
                .authenticate(&username, &password, duration)
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
