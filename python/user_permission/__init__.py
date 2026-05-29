"""Async user / group / permission management library.

The heavy lifting lives in the Rust extension module
(``user_permission._user_permission``). This Python layer wraps the
extension's async methods in ``async def`` thunks so that the awaitable
itself is constructed inside the running asyncio loop rather than at
call time. Without this, patterns like ``asyncio.run(db.connect())`` —
where ``db.connect()`` is evaluated before ``asyncio.run`` starts the
loop — would raise ``RuntimeError: no running event loop`` because
``pyo3-async-runtimes::future_into_py`` requires a running loop the
moment the Rust method is called.
"""

from __future__ import annotations

from datetime import timedelta
from pathlib import Path
from typing import Any

from . import _user_permission as _ext
from ._user_permission import (
    ALL_SCOPES,
    SCOPE_GROUPS_READ,
    SCOPE_USERS_READ,
    Group,
    ServiceClient,
    User,
    __version__,
    validate_scopes,
)

__all__ = [
    "ALL_SCOPES",
    "SCOPE_GROUPS_READ",
    "SCOPE_USERS_READ",
    "Database",
    "Group",
    "GroupManager",
    "ServiceClient",
    "ServiceClientManager",
    "User",
    "UserManager",
    "__version__",
    "serve",
    "validate_scopes",
]


class UserManager:
    """Async wrapper around the Rust extension's ``UserManager``."""

    __slots__ = ("_inner",)

    def __init__(self, inner: Any) -> None:
        self._inner = inner

    async def create(
        self,
        username: str,
        password: str,
        display_name: str = "",
        *,
        token: str | None = None,
    ) -> User:
        return await self._inner.create(username, password, display_name, token=token)

    async def get_by_id(self, user_id: int, *, token: str | None = None) -> User | None:
        return await self._inner.get_by_id(user_id, token=token)

    async def get_by_username(
        self, username: str, *, token: str | None = None
    ) -> User | None:
        return await self._inner.get_by_username(username, token=token)

    async def list_all(self, *, token: str | None = None) -> list[User]:
        return await self._inner.list_all(token=token)

    async def update(
        self,
        user_id: int,
        *,
        username: str | None = None,
        password: str | None = None,
        display_name: str | None = None,
        is_active: bool | None = None,
        token: str | None = None,
    ) -> User | None:
        return await self._inner.update(
            user_id,
            username=username,
            password=password,
            display_name=display_name,
            is_active=is_active,
            token=token,
        )

    async def delete(self, user_id: int, *, token: str | None = None) -> bool:
        return await self._inner.delete(user_id, token=token)

    async def is_admin(self, user_id: int, *, token: str | None = None) -> bool:
        return await self._inner.is_admin(user_id, token=token)

    async def set_admin(
        self, user_id: int, is_admin: bool, *, token: str | None = None
    ) -> bool:
        return await self._inner.set_admin(user_id, is_admin, token=token)


class GroupManager:
    """Async wrapper around the Rust extension's ``GroupManager``."""

    __slots__ = ("_inner",)

    def __init__(self, inner: Any) -> None:
        self._inner = inner

    async def create(
        self,
        name: str,
        description: str = "",
        *,
        is_admin: bool = False,
        token: str | None = None,
    ) -> Group:
        return await self._inner.create(
            name, description, is_admin=is_admin, token=token
        )

    async def get_by_id(
        self, group_id: int, *, token: str | None = None
    ) -> Group | None:
        return await self._inner.get_by_id(group_id, token=token)

    async def get_by_name(self, name: str, *, token: str | None = None) -> Group | None:
        return await self._inner.get_by_name(name, token=token)

    async def list_all(self, *, token: str | None = None) -> list[Group]:
        return await self._inner.list_all(token=token)

    async def list_admin_groups(self, *, token: str | None = None) -> list[Group]:
        return await self._inner.list_admin_groups(token=token)

    async def update(
        self,
        group_id: int,
        *,
        name: str | None = None,
        description: str | None = None,
        is_admin: bool | None = None,
        token: str | None = None,
    ) -> Group | None:
        return await self._inner.update(
            group_id,
            name=name,
            description=description,
            is_admin=is_admin,
            token=token,
        )

    async def delete(self, group_id: int, *, token: str | None = None) -> bool:
        return await self._inner.delete(group_id, token=token)

    async def add_user(
        self, group_id: int, user_id: int, *, token: str | None = None
    ) -> bool:
        return await self._inner.add_user(group_id, user_id, token=token)

    async def remove_user(
        self, group_id: int, user_id: int, *, token: str | None = None
    ) -> bool:
        return await self._inner.remove_user(group_id, user_id, token=token)

    async def get_members(
        self, group_id: int, *, token: str | None = None
    ) -> list[User]:
        return await self._inner.get_members(group_id, token=token)

    async def get_user_groups(
        self, user_id: int, *, token: str | None = None
    ) -> list[Group]:
        return await self._inner.get_user_groups(user_id, token=token)


class ServiceClientManager:
    """Async wrapper around the Rust extension's ``ServiceClientManager``.

    Machine-to-machine service clients are administered against the local
    backend only; calling these on a relay ``Database`` raises an error.
    """

    __slots__ = ("_inner",)

    def __init__(self, inner: Any) -> None:
        self._inner = inner

    async def create(
        self,
        name: str,
        scopes: list[str],
        *,
        expires_at: str | None = None,
    ) -> tuple[ServiceClient, str]:
        """Create a client, returning ``(client, secret)``.

        The plaintext ``secret`` is only ever returned here — the database
        stores an Argon2 hash.
        """
        return await self._inner.create(name, list(scopes), expires_at)

    async def list(self) -> list[ServiceClient]:
        return await self._inner.list()

    async def get_by_client_id(self, client_id: str) -> ServiceClient | None:
        return await self._inner.get_by_client_id(client_id)

    async def delete(self, id: int) -> bool:
        return await self._inner.delete(id)

    async def rotate_secret(self, id: int) -> str | None:
        return await self._inner.rotate_secret(id)


class Database:
    """Async user / group database with local SQLite or HTTP relay backend."""

    __slots__ = ("_inner", "_users", "_groups", "_service_clients")

    def __init__(
        self,
        backend: str | Path,
        *,
        secret: str | Path | None = None,
    ) -> None:
        self._inner = _ext.Database(backend, secret=secret)
        self._users = UserManager(self._inner.users)
        self._groups = GroupManager(self._inner.groups)
        self._service_clients = ServiceClientManager(self._inner.service_clients)

    async def connect(self) -> None:
        await self._inner.connect()

    async def close(self) -> None:
        await self._inner.close()

    async def __aenter__(self) -> "Database":
        await self._inner.connect()
        return self

    async def __aexit__(self, *exc: object) -> None:
        await self._inner.close()

    async def login(
        self,
        username: str,
        password: str,
        expires_delta: timedelta | None = None,
    ) -> str | None:
        return await self._inner.login(
            username, password, expires_delta=expires_delta
        )

    async def login_service(
        self,
        client_id: str,
        client_secret: str,
        expires_delta: timedelta | None = None,
    ) -> str | None:
        return await self._inner.login_service(
            client_id, client_secret, expires_delta=expires_delta
        )

    async def verify_token_and_get_user(self, token: str | None) -> User | None:
        return await self._inner.verify_token_and_get_user(token)

    async def bootstrap_admin_if_needed(
        self, username: str, password: str, display_name: str = ""
    ) -> User | None:
        return await self._inner.bootstrap_admin_if_needed(
            username, password, display_name
        )

    @property
    def users(self) -> UserManager:
        return self._users

    @property
    def groups(self) -> GroupManager:
        return self._groups

    @property
    def service_clients(self) -> ServiceClientManager:
        return self._service_clients


async def serve(
    *,
    database: str | Path = "user_permission.db",
    secret: str | Path = "secret.key",
    host: str = "127.0.0.1",
    port: int = 8000,
    prefix: str = "",
    webui: bool = False,
    webui_prefix: str = "/ui",
) -> None:
    """Start the bundled axum HTTP server.

    Wrapping the extension's ``serve`` in a Python ``async def`` lets
    ``asyncio.run(serve(...))`` work — the inner awaitable is only built
    once we're inside the running loop.
    """
    await _ext.serve(
        database=str(database),
        secret=str(secret),
        host=host,
        port=port,
        prefix=prefix,
        webui=webui,
        webui_prefix=webui_prefix,
    )
