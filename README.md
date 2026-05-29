# user-permission-py

![PyPI - License](https://img.shields.io/pypi/l/user-permission?cacheSeconds=0)
![PyPI - Version](https://img.shields.io/pypi/v/user-permission?cacheSeconds=0)
![Pepy Total Downloads](https://img.shields.io/pepy/dt/user-permission?cacheSeconds=0)

ユーザーとグループを管理するための非同期ライブラリ `user-permission` の **Python バインディング** です。

Rust 実装本体は別リポジトリに分離されました: **[mokuichi147/user-permission](https://github.com/mokuichi147/user-permission)**

このリポジトリは PyO3 + maturin による薄いラッパーのみを含み、PyPI パッケージ `user-permission` をビルド・公開します。

## クイックスタート (サーバーを試す)

インストール不要で、`uvx` から直接サーバーを起動できます。

```bash
uvx user-permission serve --webui
```

ブラウザで <http://127.0.0.1:8000/ui> を開くと Web 管理画面が利用できます。
オプション一覧は [サーバー起動](#サーバー起動) を参照してください。

## インストール

```bash
pip install user-permission
```

依存パッケージはありません (Rust 拡張に同梱)。Python 3.9 以降の abi3 wheel を公開しています。

ソースからビルドする場合:

```bash
uv run maturin develop          # 開発用に現在の venv に組み込む
uv run maturin build --release  # リリース wheel をビルド
```

## 使い方

### 初期化

```python
import asyncio
from user_permission import Database

async def main():
    # 初回実行時にシークレットキーを自動生成（以降はファイルから読み込み）
    async with Database("app.db", secret="secret.key") as db:
        user = await db.users.create("alice", "password123")
        group = await db.groups.create("admins")

asyncio.run(main())
```

### ユーザー管理

```python
user = await db.users.create("alice", "password123", display_name="Alice")
user = await db.users.get_by_id(1)
user = await db.users.get_by_username("alice")
users = await db.users.list_all()

await db.users.update(user.id, password="new_password")
await db.users.update(user.id, display_name="Alice Smith")
await db.users.update(user.id, is_active=False)
await db.users.delete(user.id)
```

### 認証・トークン

```python
from datetime import timedelta

token = await db.login("alice", "password123")
token = await db.login(
    "alice", "password123", expires_delta=timedelta(hours=24)
)

# トークンを検証してユーザーを解決（無効・期限切れは None）
user = await db.verify_token_and_get_user(token)
print(user.id)          # ユーザーID
print(user.username)    # ユーザー名
print(await db.users.is_admin(user.id))  # bool
```

### グループ管理

```python
group = await db.groups.create("admins", description="Administrator group")
group = await db.groups.get_by_id(1)
group = await db.groups.get_by_name("admins")
groups = await db.groups.list_all()

await db.groups.update(group.id, description="Updated description")
await db.groups.delete(group.id)
```

### グループメンバー管理

```python
await db.groups.add_user(group.id, user.id)
await db.groups.remove_user(group.id, user.id)
members = await db.groups.get_members(group.id)
groups = await db.groups.get_user_groups(user.id)
```

### サーバー起動

```python
import asyncio
from user_permission import serve

asyncio.run(serve(host="0.0.0.0", port=8001, prefix="/api", webui=True))
```

CLI からも起動できます。

```bash
user-permission serve --host 0.0.0.0 --port 8001 --prefix /api --webui
```

| オプション | デフォルト | 説明 |
|---|---|---|
| `--host` | `127.0.0.1` | バインドアドレス |
| `--port` | `8000` | バインドポート |
| `--database` | `user_permission.db` | SQLiteデータベースのパス |
| `--secret` | `secret.key` | シークレットキーファイルのパス |
| `--prefix` | (なし) | APIルートプレフィックス（例: `/api`） |
| `--webui` | 無効 | Web管理画面を有効化 |
| `--webui-prefix` | `/ui` | 管理画面のURLプレフィックス |

### リレー（中継）

`Database` に URL を渡すと、ローカル SQLite と中央サーバーを同じインターフェースで切り替えられます。

```python
from user_permission import Database

# ファイルパス → ローカル SQLite
db = Database("app.db", secret="secret.key")

# URL → リモートサーバーへ HTTP 中継
async with Database("http://localhost:8001") as db:
    token = await db.login("alice", "password123")
    users = await db.users.list_all()
```

`db.login(...)` で取得したトークンは Database が内部に保持し、以降のリクエストの `Authorization: Bearer` に自動付与されます。

**推奨: backend を意識しない実装。** 認証（`db.login` / `db.login_service`）、トークン検証（`db.verify_token_and_get_user`）、ユーザー・グループ操作はすべてローカル / リレーで同一の呼び出しで動作します。接続先 URL（またはファイルパス）を切り替えるだけで、アプリ側のコードを変えずにローカル運用と中央サーバー運用を行き来できます。

```python
# どちらの backend でも同じコードが動く
async def authenticate(db: Database, username: str, password: str):
    # login 失敗時は None、verify_token_and_get_user は None を渡すと None を返す
    token = await db.login(username, password)
    return await db.verify_token_and_get_user(token)
```

> サービスクライアントの**管理操作**だけは例外でローカル専用です（後述）。

### サービスクライアント認証（client-credentials）

サービス間連携向けに、平文パスワードを持たずに**読み取り専用**のスコープ付きトークンを発行できます。
クライアントには `users:read` / `groups:read` のスコープのみ付与でき、書き込みや管理操作はできません。

```python
from user_permission import Database, SCOPE_USERS_READ

# 管理側（ローカル）でサービスクライアントを発行。secret は発行時のみ取得可能。
async with Database("app.db", secret="secret.key") as db:
    client, secret = await db.service_clients.create("reader", [SCOPE_USERS_READ])

# リレー側はサービストークンでログインし、スコープ内のみ読み取れる。
async with Database("http://localhost:8001") as relay:
    await relay.login_service(client.client_id, secret)
    users = await relay.users.list_all()  # users:read があるので OK
```

> **注意: 管理操作はローカル backend 専用です。** `service_clients.create` / `list` / `get_by_client_id` / `delete` / `rotate_secret` はリレー（URL）backend で呼ぶとエラーになります（サービスクライアントの発行・管理は中央サーバー側で行う設計のため）。一方、サービス認証（`db.login_service(...)`）はローカル / リレーのどちらでも動作します。

### バックエンド非依存のユーティリティ

ローカル / リレーのどちらでも同じ呼び出しで動作します。

```python
# トークンを検証してユーザーを解決（無効・期限切れ・サービストークンは None）
user = await db.verify_token_and_get_user(token)

# 管理者が不在なら作成して昇格（リレーでは no-op）
await db.bootstrap_admin_if_needed("admin", "password123")
```

## REST API・管理者ロール・スキーマ

REST エンドポイント仕様、`groups.is_admin` による管理者ロールの扱い、データベーススキーマは
[Rust リポジトリの README](https://github.com/mokuichi147/user-permission#readme) を参照してください。

## 開発

```bash
# Python wheel をビルドして現在の venv に組み込む
uv run maturin develop

# Python 統合テスト
uv run --with pytest --with pytest-asyncio pytest tests/python

# リリース wheel をビルド
uv run maturin build --release
```

## リリース

`pyproject.toml` のバージョンを更新し、`v` で始まる Git タグを push すると GitHub Actions が
全プラットフォームの wheel をビルドして PyPI に公開します (詳細は `.github/workflows/release.yml`)。

## ライセンス

MIT OR Apache-2.0
