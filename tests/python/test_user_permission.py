"""Python integration tests for the user-permission Rust extension.

Run with:
    maturin develop   # build & install into the current venv
    pytest tests/python
"""
from __future__ import annotations

import os
import tempfile
from datetime import timedelta

import pytest

import user_permission
from user_permission import Database


@pytest.fixture
def db_paths():
    with tempfile.TemporaryDirectory() as tmp:
        yield os.path.join(tmp, "app.db"), os.path.join(tmp, "secret.key")


@pytest.mark.asyncio
async def test_version_exposed():
    assert user_permission.__version__ == "0.6.0"


@pytest.mark.asyncio
async def test_first_user_is_admin(db_paths):
    db_path, secret = db_paths
    async with Database(db_path, secret=secret) as db:
        alice = await db.users.create("alice", "pw", "Alice")
        assert alice.username == "alice"
        assert await db.users.is_admin(alice.id) is True

        bob = await db.users.create("bob", "pw", "Bob")
        assert await db.users.is_admin(bob.id) is False


@pytest.mark.asyncio
async def test_user_crud_round_trip(db_paths):
    db_path, secret = db_paths
    async with Database(db_path, secret=secret) as db:
        alice = await db.users.create("alice", "pw", "Alice")
        assert (await db.users.get_by_id(alice.id)).username == "alice"
        assert (await db.users.get_by_username("alice")).id == alice.id
        assert len(await db.users.list_all()) == 1

        updated = await db.users.update(alice.id, display_name="Alice Smith")
        assert updated.display_name == "Alice Smith"

        assert await db.users.delete(alice.id) is True
        assert await db.users.get_by_id(alice.id) is None


@pytest.mark.asyncio
async def test_login_and_verify(db_paths):
    db_path, secret = db_paths
    async with Database(db_path, secret=secret) as db:
        alice = await db.users.create("alice", "pw", "")
        token = await db.login("alice", "pw", expires_delta=timedelta(hours=1))
        assert token is not None
        resolved = await db.verify_token_and_get_user(token)
        assert resolved is not None
        assert resolved.username == "alice"
        assert await db.users.is_admin(alice.id) is True

        assert await db.login("alice", "bad") is None
        assert await db.login("nobody", "pw") is None


@pytest.mark.asyncio
async def test_groups_and_membership(db_paths):
    db_path, secret = db_paths
    async with Database(db_path, secret=secret) as db:
        await db.users.create("alice", "pw", "")  # admin
        bob = await db.users.create("bob", "pw", "")

        editors = await db.groups.create("editors", "Editors")
        assert editors.is_admin is False

        assert await db.groups.add_user(editors.id, bob.id) is True
        members = await db.groups.get_members(editors.id)
        assert [m.username for m in members] == ["bob"]

        bob_groups = await db.groups.get_user_groups(bob.id)
        assert [g.name for g in bob_groups] == ["editors"]

        assert await db.groups.remove_user(editors.id, bob.id) is True
        assert await db.groups.get_members(editors.id) == []


@pytest.mark.asyncio
async def test_promote_and_demote(db_paths):
    db_path, secret = db_paths
    async with Database(db_path, secret=secret) as db:
        await db.users.create("alice", "pw", "")  # auto-admin
        bob = await db.users.create("bob", "pw", "")
        assert await db.users.is_admin(bob.id) is False
        await db.users.set_admin(bob.id, True)
        assert await db.users.is_admin(bob.id) is True
        await db.users.set_admin(bob.id, False)
        assert await db.users.is_admin(bob.id) is False


# --- Regression tests for the "no running event loop" trap. ---
#
# The Rust extension exposes awaitables via pyo3-async-runtimes' `future_into_py`,
# which captures the running asyncio loop *at the moment the Rust method is
# called* — not at await-time. Without the Python wrapper layer, passing an
# extension awaitable straight to `asyncio.run` would raise
# `RuntimeError: no running event loop`. These tests pin down that the
# wrapper layer keeps the natural Python patterns working.


def test_asyncio_run_direct_connect(db_paths):
    """`asyncio.run(db.connect())` must work — db.connect() must build its
    awaitable inside the loop, not at evaluation time."""
    import asyncio

    db_path, secret = db_paths
    db = Database(db_path, secret=secret)
    asyncio.run(db.connect())
    asyncio.run(db.close())


def test_asyncio_run_direct_user_create(db_paths):
    """A single-shot `asyncio.run(...)` with a manager call must also work."""
    import asyncio

    db_path, secret = db_paths
    db = Database(db_path, secret=secret)
    asyncio.run(db.connect())
    user = asyncio.run(db.users.create("alice", "pw", "Alice"))
    assert user.username == "alice"
    asyncio.run(db.close())


@pytest.mark.asyncio
async def test_relay_per_call_token(db_paths):
    """Relay backend: per-call ``token=`` 引数が内部保持トークンより優先される。

    1 つの ``Database`` (relay) を共有しながら、リクエストごとに異なる
    ユーザーのトークンを ``token=`` で pass-through する FastAPI 風シナリオ
    の最小再現。
    """
    import asyncio

    db_path, secret = db_paths
    server_db = Database(db_path, secret=secret)
    await server_db.connect()
    # 1 人目は admin、2 人目は一般ユーザー。
    await server_db.users.create("alice", "pw", "Alice")
    await server_db.users.create("bob", "pw", "Bob")

    # バックグラウンドで bundled axum server を起動。
    server_task = asyncio.create_task(
        user_permission.serve(
            database=db_path,
            secret=secret,
            host="127.0.0.1",
            port=18745,
            webui=False,
        )
    )
    await asyncio.sleep(0.5)
    try:
        # Relay クライアントを 1 つだけ作る (login() は呼ばない)。
        relay = Database("http://127.0.0.1:18745")
        await relay.connect()

        # 各ユーザーのトークンを (admin の) server 経由で発行。
        alice_token = await server_db.login("alice", "pw")
        bob_token = await server_db.login("bob", "pw")
        assert alice_token and bob_token

        # 共有 relay インスタンスから per-call token で切り替えながら呼ぶ。
        users_via_alice = await relay.users.list_all(token=alice_token)
        assert {u.username for u in users_via_alice} == {"alice", "bob"}

        users_via_bob = await relay.users.list_all(token=bob_token)
        assert {u.username for u in users_via_bob} == {"alice", "bob"}

        # token を渡さなければ内部保持なし → 401 で Relay エラー。
        with pytest.raises(Exception):
            await relay.users.list_all()

        await relay.close()
    finally:
        server_task.cancel()
        try:
            await server_task
        except (asyncio.CancelledError, BaseException):
            pass
        await server_db.close()


@pytest.mark.asyncio
async def test_local_backend_verifies_per_call_token(db_paths):
    """Local backend: ``token=`` を渡すと JWT が検証される (v0.2.2)。"""
    db_path, secret = db_paths
    async with Database(db_path, secret=secret) as db:
        alice = await db.users.create("alice", "pw", "Alice")

        # 有効な JWT を渡せばアクセスできる。
        token = await db.login("alice", "pw")
        assert token is not None
        fetched = await db.users.get_by_id(alice.id, token=token)
        assert fetched.id == alice.id

        # 不正な JWT はエラーになる。
        with pytest.raises(Exception):
            await db.users.get_by_id(alice.id, token="not-a-valid-jwt")

        # token=None は従来どおり通る。
        assert (await db.users.get_by_id(alice.id)).id == alice.id


@pytest.mark.asyncio
async def test_local_backend_without_secret_rejects_token(db_paths):
    """secret 未設定の local backend に token を渡すと拒否される (v0.2.2)。"""
    db_path, _secret = db_paths
    async with Database(db_path) as db:
        alice = await db.users.create("alice", "pw", "Alice")

        # secret 未設定なので token を渡すとエラー。
        with pytest.raises(Exception):
            await db.users.get_by_id(alice.id, token="anything")

        # None なら従来通り通る。
        assert (await db.users.get_by_id(alice.id)).id == alice.id


@pytest.mark.asyncio
async def test_bootstrap_admin_if_needed(db_paths):
    """local backend: 管理者不在なら作成・昇格し、存在すれば no-op (v0.2.5)。"""
    db_path, secret = db_paths
    async with Database(db_path, secret=secret) as db:
        # 管理者がいないので作成され、昇格されて返る。
        admin = await db.bootstrap_admin_if_needed("admin", "pw", "Admin")
        assert admin is not None
        assert admin.username == "admin"
        assert await db.users.is_admin(admin.id) is True

        # すでに管理者がいるので 2 回目は no-op (None)。
        assert await db.bootstrap_admin_if_needed("admin2", "pw") is None
        assert await db.users.get_by_username("admin2") is None


@pytest.mark.asyncio
async def test_verify_token_and_get_user_local(db_paths):
    """local backend: トークンを検証してユーザーを解決する (v0.2.5)。"""
    db_path, secret = db_paths
    async with Database(db_path, secret=secret) as db:
        alice = await db.users.create("alice", "pw", "Alice")
        token = await db.login("alice", "pw")
        assert token is not None

        resolved = await db.verify_token_and_get_user(token)
        assert resolved is not None
        assert resolved.id == alice.id

        # 無効なトークンは None。
        assert await db.verify_token_and_get_user("not-a-jwt") is None

        # None を渡しても例外にならず None（login 失敗をそのまま渡せる）。
        assert await db.verify_token_and_get_user(None) is None


@pytest.mark.asyncio
async def test_service_client_lifecycle(db_paths):
    """local backend: サービスクライアントの発行・認証・失効 (v0.2.4)。"""
    db_path, secret = db_paths
    async with Database(db_path, secret=secret) as db:
        await db.users.create("alice", "pw", "Alice")  # admin

        client, plaintext = await db.service_clients.create(
            "reader", [user_permission.SCOPE_USERS_READ]
        )
        assert client.scopes == [user_permission.SCOPE_USERS_READ]
        assert client.is_active is True
        assert plaintext.startswith("ups_")

        # 一覧・client_id 解決。
        assert [c.client_id for c in await db.service_clients.list()] == [
            client.client_id
        ]
        fetched = await db.service_clients.get_by_client_id(client.client_id)
        assert fetched is not None and fetched.id == client.id

        # 正しい secret でスコープ付きサービストークンを取得できる。
        token = await db.login_service(client.client_id, plaintext)
        assert token is not None
        # サービストークンはユーザーに解決できない。
        assert await db.verify_token_and_get_user(token) is None

        # 誤った secret は None。
        assert await db.login_service(client.client_id, "ups_wrong") is None

        # rotate 後は旧 secret が無効、新 secret が有効。
        new_secret = await db.service_clients.rotate_secret(client.id)
        assert new_secret is not None and new_secret != plaintext
        assert await db.login_service(client.client_id, plaintext) is None
        assert await db.login_service(client.client_id, new_secret) is not None

        # 削除後は認証不可。
        assert await db.service_clients.delete(client.id) is True
        assert await db.login_service(client.client_id, new_secret) is None


@pytest.mark.asyncio
async def test_unknown_scope_rejected(db_paths):
    """未知スコープは create でも validate_scopes でも拒否される (v0.2.4)。"""
    db_path, secret = db_paths
    user_permission.validate_scopes([user_permission.SCOPE_USERS_READ])
    with pytest.raises(Exception):
        user_permission.validate_scopes(["users:write"])

    async with Database(db_path, secret=secret) as db:
        await db.users.create("alice", "pw", "Alice")
        with pytest.raises(Exception):
            await db.service_clients.create("bad", ["users:write"])


@pytest.mark.asyncio
async def test_relay_client_credentials(db_paths):
    """relay backend: client-credentials でログインしスコープ内のみ読める (v0.2.4)。"""
    import asyncio

    db_path, secret = db_paths
    server_db = Database(db_path, secret=secret)
    await server_db.connect()
    await server_db.users.create("alice", "pw", "Alice")  # admin
    client, plaintext = await server_db.service_clients.create(
        "reader", [user_permission.SCOPE_USERS_READ]
    )

    server_task = asyncio.create_task(
        user_permission.serve(
            database=db_path,
            secret=secret,
            host="127.0.0.1",
            port=18747,
            webui=False,
        )
    )
    await asyncio.sleep(0.5)
    try:
        relay = Database("http://127.0.0.1:18747")
        await relay.connect()

        # client-credentials でログイン (内部にサービストークンを保持)。
        token = await relay.login_service(client.client_id, plaintext)
        assert token

        # users:read スコープがあるので /users は読める。
        users = await relay.users.list_all()
        assert {u.username for u in users} == {"alice"}

        # groups:read は付与していないので 403 で失敗する。
        with pytest.raises(Exception):
            await relay.groups.list_all()

        await relay.close()
    finally:
        server_task.cancel()
        try:
            await server_task
        except (asyncio.CancelledError, BaseException):
            pass
        await server_db.close()


@pytest.mark.asyncio
async def test_relay_get_by_username(db_paths):
    """Relay backend: ``get_by_username`` が動作する (v0.2.3)。

    v0.2.2 以前は relay backend では未対応でエラーになっていたが、
    v0.2.3 で ``/users?username=`` 経由で解決できるようになった。
    """
    import asyncio

    db_path, secret = db_paths
    server_db = Database(db_path, secret=secret)
    await server_db.connect()
    alice = await server_db.users.create("alice", "pw", "Alice")
    await server_db.users.create("bob", "pw", "Bob")

    server_task = asyncio.create_task(
        user_permission.serve(
            database=db_path,
            secret=secret,
            host="127.0.0.1",
            port=18746,
            webui=False,
        )
    )
    await asyncio.sleep(0.5)
    try:
        relay = Database("http://127.0.0.1:18746")
        await relay.connect()

        alice_token = await server_db.login("alice", "pw")
        assert alice_token

        # 既存ユーザーは relay backend 経由で解決できる。
        found = await relay.users.get_by_username("alice", token=alice_token)
        assert found is not None
        assert found.id == alice.id
        assert found.username == "alice"

        # 存在しないユーザーは None。
        missing = await relay.users.get_by_username("nobody", token=alice_token)
        assert missing is None

        await relay.close()
    finally:
        server_task.cancel()
        try:
            await server_task
        except (asyncio.CancelledError, BaseException):
            pass
        await server_db.close()
