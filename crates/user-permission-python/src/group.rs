use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use user_permission_core::{Group, GroupUpdate};

use crate::database::SharedDb;
use crate::error::map_core_err;
use crate::user::PyUser;

#[pyclass(module = "user_permission", name = "Group", get_all)]
#[derive(Clone)]
pub struct PyGroup {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub is_admin: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<Group> for PyGroup {
    fn from(g: Group) -> Self {
        Self {
            id: g.id,
            name: g.name,
            description: g.description,
            is_admin: g.is_admin,
            created_at: g.created_at,
            updated_at: g.updated_at,
        }
    }
}

#[pymethods]
impl PyGroup {
    fn __repr__(&self) -> String {
        format!(
            "Group(id={}, name={:?}, is_admin={})",
            self.id, self.name, self.is_admin
        )
    }
}

#[pyclass(module = "user_permission", name = "GroupManager", unsendable)]
pub struct PyGroupManager {
    db: SharedDb,
}

impl PyGroupManager {
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

#[pymethods]
impl PyGroupManager {
    #[pyo3(signature = (name, description="", *, is_admin=false, token=None))]
    fn create<'py>(
        &self,
        py: Python<'py>,
        name: String,
        description: &str,
        is_admin: bool,
        token: Option<String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let db = get_db(&self.db)?;
        let description = description.to_string();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let group = db
                .groups()
                .create(&name, &description, is_admin, token.as_deref())
                .await
                .map_err(map_core_err)?;
            Python::with_gil(|py| Ok(PyGroup::from(group).into_py(py)))
        })
    }

    #[pyo3(signature = (group_id, *, token=None))]
    fn get_by_id<'py>(
        &self,
        py: Python<'py>,
        group_id: i64,
        token: Option<String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let db = get_db(&self.db)?;
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let g = db
                .groups()
                .get_by_id(group_id, token.as_deref())
                .await
                .map_err(map_core_err)?;
            Python::with_gil(|py| {
                Ok(match g {
                    Some(g) => PyGroup::from(g).into_py(py),
                    None => py.None(),
                })
            })
        })
    }

    #[pyo3(signature = (name, *, token=None))]
    fn get_by_name<'py>(
        &self,
        py: Python<'py>,
        name: String,
        token: Option<String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let db = get_db(&self.db)?;
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let g = db
                .groups()
                .get_by_name(&name, token.as_deref())
                .await
                .map_err(map_core_err)?;
            Python::with_gil(|py| {
                Ok(match g {
                    Some(g) => PyGroup::from(g).into_py(py),
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
            let groups = db
                .groups()
                .list_all(token.as_deref())
                .await
                .map_err(map_core_err)?;
            Python::with_gil(|py| {
                Ok(groups
                    .into_iter()
                    .map(PyGroup::from)
                    .collect::<Vec<_>>()
                    .into_py(py))
            })
        })
    }

    #[pyo3(signature = (*, token=None))]
    fn list_admin_groups<'py>(
        &self,
        py: Python<'py>,
        token: Option<String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let db = get_db(&self.db)?;
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let groups = db
                .groups()
                .list_admin_groups(token.as_deref())
                .await
                .map_err(map_core_err)?;
            Python::with_gil(|py| {
                Ok(groups
                    .into_iter()
                    .map(PyGroup::from)
                    .collect::<Vec<_>>()
                    .into_py(py))
            })
        })
    }

    #[pyo3(signature = (group_id, *, name=None, description=None, is_admin=None, token=None))]
    fn update<'py>(
        &self,
        py: Python<'py>,
        group_id: i64,
        name: Option<String>,
        description: Option<String>,
        is_admin: Option<bool>,
        token: Option<String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let db = get_db(&self.db)?;
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let g = db
                .groups()
                .update(
                    group_id,
                    GroupUpdate {
                        name,
                        description,
                        is_admin,
                    },
                    token.as_deref(),
                )
                .await
                .map_err(map_core_err)?;
            Python::with_gil(|py| {
                Ok(match g {
                    Some(g) => PyGroup::from(g).into_py(py),
                    None => py.None(),
                })
            })
        })
    }

    #[pyo3(signature = (group_id, *, token=None))]
    fn delete<'py>(
        &self,
        py: Python<'py>,
        group_id: i64,
        token: Option<String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let db = get_db(&self.db)?;
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            db.groups()
                .delete(group_id, token.as_deref())
                .await
                .map_err(map_core_err)
        })
    }

    #[pyo3(signature = (group_id, user_id, *, token=None))]
    fn add_user<'py>(
        &self,
        py: Python<'py>,
        group_id: i64,
        user_id: i64,
        token: Option<String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let db = get_db(&self.db)?;
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            db.groups()
                .add_user(group_id, user_id, token.as_deref())
                .await
                .map_err(map_core_err)
        })
    }

    #[pyo3(signature = (group_id, user_id, *, token=None))]
    fn remove_user<'py>(
        &self,
        py: Python<'py>,
        group_id: i64,
        user_id: i64,
        token: Option<String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let db = get_db(&self.db)?;
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            db.groups()
                .remove_user(group_id, user_id, token.as_deref())
                .await
                .map_err(map_core_err)
        })
    }

    #[pyo3(signature = (group_id, *, token=None))]
    fn get_members<'py>(
        &self,
        py: Python<'py>,
        group_id: i64,
        token: Option<String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let db = get_db(&self.db)?;
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let members = db
                .groups()
                .get_members(group_id, token.as_deref())
                .await
                .map_err(map_core_err)?;
            Python::with_gil(|py| {
                Ok(members
                    .into_iter()
                    .map(PyUser::from)
                    .collect::<Vec<_>>()
                    .into_py(py))
            })
        })
    }

    #[pyo3(signature = (user_id, *, token=None))]
    fn get_user_groups<'py>(
        &self,
        py: Python<'py>,
        user_id: i64,
        token: Option<String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let db = get_db(&self.db)?;
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let groups = db
                .groups()
                .get_user_groups(user_id, token.as_deref())
                .await
                .map_err(map_core_err)?;
            Python::with_gil(|py| {
                Ok(groups
                    .into_iter()
                    .map(PyGroup::from)
                    .collect::<Vec<_>>()
                    .into_py(py))
            })
        })
    }
}
