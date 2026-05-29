from datetime import timedelta
from pathlib import Path
from typing import Any

__version__: str

SCOPE_USERS_READ: str
SCOPE_GROUPS_READ: str
ALL_SCOPES: list[str]

class User:
    id: int
    username: str
    display_name: str
    is_active: bool
    created_at: str
    updated_at: str

class Group:
    id: int
    name: str
    description: str
    is_admin: bool
    created_at: str
    updated_at: str

class UserManager:
    async def create(
        self, username: str, password: str, display_name: str = ""
    ) -> User: ...
    async def get_by_id(self, user_id: int) -> User | None: ...
    async def get_by_username(self, username: str) -> User | None: ...
    async def list_all(self) -> list[User]: ...
    async def update(
        self,
        user_id: int,
        *,
        username: str | None = None,
        password: str | None = None,
        display_name: str | None = None,
        is_active: bool | None = None,
    ) -> User | None: ...
    async def delete(self, user_id: int) -> bool: ...
    async def is_admin(self, user_id: int) -> bool: ...
    async def set_admin(self, user_id: int, is_admin: bool) -> bool: ...

class GroupManager:
    async def create(
        self,
        name: str,
        description: str = "",
        *,
        is_admin: bool = False,
    ) -> Group: ...
    async def get_by_id(self, group_id: int) -> Group | None: ...
    async def get_by_name(self, name: str) -> Group | None: ...
    async def list_all(self) -> list[Group]: ...
    async def list_admin_groups(self) -> list[Group]: ...
    async def update(
        self,
        group_id: int,
        *,
        name: str | None = None,
        description: str | None = None,
        is_admin: bool | None = None,
    ) -> Group | None: ...
    async def delete(self, group_id: int) -> bool: ...
    async def add_user(self, group_id: int, user_id: int) -> bool: ...
    async def remove_user(self, group_id: int, user_id: int) -> bool: ...
    async def get_members(self, group_id: int) -> list[User]: ...
    async def get_user_groups(self, user_id: int) -> list[Group]: ...

class ServiceClient:
    id: int
    client_id: str
    name: str
    scopes: list[str]
    is_active: bool
    expires_at: str | None
    created_at: str
    last_used_at: str | None

class ServiceClientManager:
    async def create(
        self,
        name: str,
        scopes: list[str],
        *,
        expires_at: str | None = None,
    ) -> tuple[ServiceClient, str]: ...
    async def list(self) -> list[ServiceClient]: ...
    async def get_by_client_id(self, client_id: str) -> ServiceClient | None: ...
    async def delete(self, id: int) -> bool: ...
    async def rotate_secret(self, id: int) -> str | None: ...

class Database:
    def __init__(
        self,
        backend: str | Path,
        *,
        secret: str | Path | None = None,
    ) -> None: ...
    async def connect(self) -> None: ...
    async def close(self) -> None: ...
    async def __aenter__(self) -> "Database": ...
    async def __aexit__(self, *exc: Any) -> None: ...
    async def login(
        self,
        username: str,
        password: str,
        expires_delta: timedelta | None = ...,
    ) -> str | None: ...
    async def login_service(
        self,
        client_id: str,
        client_secret: str,
        expires_delta: timedelta | None = ...,
    ) -> str | None: ...
    async def verify_token_and_get_user(self, token: str) -> User | None: ...
    async def bootstrap_admin_if_needed(
        self, username: str, password: str, display_name: str = ""
    ) -> User | None: ...
    @property
    def users(self) -> UserManager: ...
    @property
    def groups(self) -> GroupManager: ...
    @property
    def service_clients(self) -> ServiceClientManager: ...

def validate_scopes(scopes: list[str]) -> None: ...
async def serve(
    *,
    database: str = "user_permission.db",
    secret: str = "secret.key",
    host: str = "127.0.0.1",
    port: int = 8000,
    prefix: str = "",
    webui: bool = False,
    webui_prefix: str = "/ui",
) -> None: ...
