# UserPermission (Python bindings)

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
maturin develop          # 開発用に現在の venv に組み込む
maturin build --release  # リリース wheel をビルド
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

token = await db.users.authenticate("alice", "password123")
token = await db.users.authenticate(
    "alice", "password123", expires_delta=timedelta(hours=24)
)

payload = db.token_manager.verify_token(token)
print(payload["sub"])        # ユーザーID（文字列）
print(payload["username"])   # ユーザー名
print(payload["is_admin"])   # bool
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

## REST API・管理者ロール・スキーマ

REST エンドポイント仕様、`groups.is_admin` による管理者ロールの扱い、データベーススキーマは
[Rust リポジトリの README](https://github.com/mokuichi147/user-permission#readme) を参照してください。

## 開発

```bash
# Python wheel をビルドして現在の venv に組み込む
maturin develop

# Python 統合テスト
pip install pytest pytest-asyncio
pytest tests/python

# リリース wheel をビルド
maturin build --release
```

## リリース

`pyproject.toml` のバージョンを更新し、`v` で始まる Git タグを push すると GitHub Actions が
全プラットフォームの wheel をビルドして PyPI に公開します (詳細は `.github/workflows/release.yml`)。

## ライセンス

MIT OR Apache-2.0
