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
    assert user_permission.__version__ == "0.5.0"


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
async def test_authenticate_and_verify(db_paths):
    db_path, secret = db_paths
    async with Database(db_path, secret=secret) as db:
        await db.users.create("alice", "pw", "")
        token = await db.users.authenticate(
            "alice", "pw", expires_delta=timedelta(hours=1)
        )
        assert token is not None
        claims = db.token_manager.verify_token(token)
        assert claims["username"] == "alice"
        assert claims["is_admin"] is True

        assert await db.users.authenticate("alice", "bad") is None
        assert await db.users.authenticate("nobody", "pw") is None


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


@pytest.mark.asyncio
async def test_password_helpers():
    h = user_permission.hash_password("hello")
    assert user_permission.verify_password("hello", h) is True
    assert user_permission.verify_password("world", h) is False
    # PHC string format
    assert h.startswith("$argon2id$")


@pytest.mark.asyncio
async def test_load_or_create_secret(tmp_path):
    path = tmp_path / "nested" / "secret.key"
    first = user_permission.load_or_create_secret(str(path))
    second = user_permission.load_or_create_secret(str(path))
    assert first == second
    assert len(first) == 64


@pytest.mark.asyncio
async def test_token_manager_round_trip(db_paths):
    _db_path, secret = db_paths
    tm = user_permission.TokenManager.from_file(secret)
    token = tm.create_token(
        42, "alice", expires_delta=timedelta(minutes=5), extra_claims={"role": "x"}
    )
    claims = tm.verify_token(token)
    assert claims["sub"] == "42"
    assert claims["username"] == "alice"
    assert claims["role"] == "x"


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
        alice_token = await server_db.users.authenticate("alice", "pw")
        bob_token = await server_db.users.authenticate("bob", "pw")
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
