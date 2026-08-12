#!/usr/bin/env bash
# 7 Days to Die 用VMを初回起動時に構成するプロビジョニングスクリプト。
set -euo pipefail
# apt の対話プロンプトを無効化し、起動時に自動実行できるようにする。
export DEBIAN_FRONTEND=noninteractive

# 完了マーカーがある場合は再実行せず、既存環境を変更しない。
if [ -f /var/lib/seven-days-provisioned ]; then exit 0; fi

# SteamCMD と、データディスクを扱うためのツールをインストールする。
apt-get update
## steamcmdはUbuntuのaptリポジトリ区分の一つmultiverse(ライセンスや再配布条件に制限があるソフトウェア)に分類されるため、multiverseを有効にする
apt-get install -y software-properties-common
add-apt-repository -y multiverse
## steamcmdがi386(32bit x86)版として提供されているため、64-bit Ubuntu VM環境で必要
dpkg --add-architecture i386
apt-get update
## Steam ライセンス確認への回答を先に登録して、apt の停止を防ぐ。
printf 'steam steam/question select I AGREE\nsteam steam/license note \n' | debconf-set-selections
## curlはGCP VMのメタデータ取得に使用
apt-get install -y curl steamcmd xfsprogs telnet logrotate

# サーバープロセス専用の権限を作成する（既存ならそのまま利用する）。
id seven-days >/dev/null 2>&1 || useradd --system --create-home --shell /usr/sbin/nologin seven-days

# 追加の永続データ用ディスクをXFSで初期化し、再起動後も同じ場所へマウントする。
## デバイス(ディスク操作用 - ゲームデータを書き込むと、XFSの構造を壊したり、データを上書きする危険があるので直接触らない)
DEVICE=/dev/disk/by-id/google-seven-days-data
## マウントポイント(ファイル操作用)
MOUNT=/srv/seven-days-data
mkdir -p "$MOUNT"
## XFSとしてフォーマットし、ディスク内部にUUIDを保存
if ! blkid "$DEVICE" >/dev/null 2>&1; then mkfs.xfs "$DEVICE"; fi
## UUIDを取得
UUID=$(blkid -s UUID -o value "$DEVICE")
## fstab（マウント設定記録ファイル）にマウント情報を記録
##
## UUID=...                マウントするディスク
## /srv/seven-days-data    マウント先
## xfs                     ファイルシステムの種類
## defaults,nofail         マウントオプション
## 0                       dump対象外
## 2                       ファイルシステム検査の順番
grep -q "$UUID" /etc/fstab || printf 'UUID=%s %s xfs defaults,nofail 0 2\n' "$UUID" "$MOUNT" >> /etc/fstab
## ゲームの本体とセーブデータ
##
## ゲーム本体
## /opt/seven-days
##
## セーブデータ
## /srv/seven-days-data
##    ├── Saves
##    ├── GeneratedWorlds
##    ├── Mods
##    └── serverconfig.xml
mount "$MOUNT" || mount -a
mkdir -p "$MOUNT"/{Saves,GeneratedWorlds,Mods} /opt/seven-days
chown -R seven-days:seven-days "$MOUNT" /opt/seven-days

# ゲームサーバー本体を専用ユーザー(seven-days)でインストール・検証する。
# Steam側の一時的な app 情報取得失敗に備え、失敗時だけ最大3回まで再試行する。
STEAMCMD_SUCCEEDED=false
for attempt in 1 2 3; do
  if runuser -u seven-days -- /usr/games/steamcmd +force_install_dir /opt/seven-days +login anonymous +app_update 294420 validate +quit; then
    STEAMCMD_SUCCEEDED=true
    break
  fi
  printf 'SteamCMD の実行に失敗しました（%s/3）。10秒後に再試行します\n' "$attempt" >&2
  sleep 10
done
if [ "$STEAMCMD_SUCCEEDED" != true ]; then
  printf 'SteamCMD が3回連続で失敗したため、プロビジョニングを中止します\n' >&2
  exit 1
fi

# Compute Engine のメタデータから、停止・バックアップ用スクリプトを取得する。
METADATA=http://metadata.google.internal/computeMetadata/v1/instance/attributes
## metadata_file(): GCP VMのメタデータサーバーから設定済みの値を取得する関数
## 1. GCPメタデータから値を取得
## 2. Base64としてデコード
## 3. 指定されたファイルへ保存
metadata_file() { curl -fsS -H 'Metadata-Flavor: Google' "$METADATA/$1" | base64 -d >"$2"; }
metadata_file seven-days-safe-stop /usr/local/sbin/seven-days-safe-stop
metadata_file seven-days-backup /usr/local/sbin/seven-days-backup
metadata_file seven-days-backup-bucket /etc/seven-days-backup-bucket
chmod 0755 /usr/local/sbin/seven-days-safe-stop /usr/local/sbin/seven-days-backup
chmod 0644 /etc/seven-days-backup-bucket

# 初回だけメタデータにある設定テンプレートを永続ディスクへ配置する。
## 運用中に変更される設定ファイルなので、初回のみ配置
if [ ! -f "$MOUNT/serverconfig.xml" ]; then metadata_file seven-days-config "$MOUNT/serverconfig.xml"; fi

# テンプレートのプレースホルダーをVM内で生成した秘密値へ置換する。
# 値は標準出力へ出さず、rootだけが読めるファイルにも同期して停止処理から利用する。
python3 - "$MOUNT/serverconfig.xml" "$MOUNT/server-password" "$MOUNT/telnet-password" <<'PY'
import os
import secrets
import stat
import sys
import xml.etree.ElementTree as ET

config_path, server_password_path, telnet_password_path = sys.argv[1:]
tree = ET.parse(config_path)
root = tree.getroot()
properties = {prop.get("name"): prop for prop in root.findall("property")}
config_stat = os.stat(config_path)
changed = False


def write_secret(path: str, value: str) -> None:
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(descriptor, "w", encoding="utf-8") as secret_file:
        secret_file.write(value)
        secret_file.write("\n")
    if os.geteuid() == 0:
        os.chown(path, 0, 0)
    os.chmod(path, 0o600)


def configure_password(property_name: str, password_path: str) -> None:
    global changed
    prop = properties.get(property_name)
    if prop is None:
        raise RuntimeError(f"serverconfig.xml に {property_name} がありません")

    value = prop.get("value", "")
    if value == "CHANGE_BEFORE_START":
        try:
            with open(password_path, encoding="utf-8") as secret_file:
                value = secret_file.read().strip()
        except FileNotFoundError:
            value = ""
        if not value:
            value = secrets.token_hex(16)
        prop.set("value", value)
        changed = True

    write_secret(password_path, value)


configure_password("ServerPassword", server_password_path)
configure_password("TelnetPassword", telnet_password_path)

if changed:
    temporary_path = f"{config_path}.tmp"
    tree.write(temporary_path, encoding="utf-8", xml_declaration=True)
    os.chown(temporary_path, config_stat.st_uid, config_stat.st_gid)
    os.chmod(temporary_path, stat.S_IMODE(config_stat.st_mode))
    os.replace(temporary_path, config_path)
PY

# systemd でサーバーを管理し、停止時には安全停止スクリプトを呼び出す。
cat >/etc/systemd/system/seven-days.service <<'UNIT'
[Unit]
Description=7 Days to Die Dedicated Server
After=network-online.target srv-seven\x2ddays\x2ddata.mount
RequiresMountsFor=/srv/seven-days-data

[Service]
Type=simple
User=seven-days
WorkingDirectory=/opt/seven-days
ExecCondition=/bin/sh -c '! grep -q CHANGE_BEFORE_START /srv/seven-days-data/serverconfig.xml'
ExecStart=/opt/seven-days/startserver.sh -configfile=/srv/seven-days-data/serverconfig.xml -UserDataFolder=/srv/seven-days-data
ExecStop=+/usr/local/sbin/seven-days-safe-stop
TimeoutStopSec=180
Restart=on-failure

[Install]
WantedBy=multi-user.target
UNIT

# unit を登録する。
systemctl daemon-reload

# プレースホルダーが残る場合は起動せず、設定変更後の再実行に備える。
SERVER_STARTED=false
if grep -q 'CHANGE_BEFORE_START' "$MOUNT/serverconfig.xml"; then
  printf 'serverconfig.xml の CHANGE_BEFORE_START を変更してから再実行してください\n' >&2
else
  systemctl enable seven-days.service
  systemctl start seven-days.service
  # systemd の active に加え、外部接続に使う TCP 26900 の待受開始まで最大10分待つ。
  SERVER_READY=false
  for _ in $(seq 1 120); do
    if ss -H -lnt 'sport = :26900' | grep -q .; then
      SERVER_READY=true
      break
    fi
    if ! systemctl is-active --quiet seven-days.service; then break; fi
    sleep 5
  done
  if [ "$SERVER_READY" != true ]; then
    printf '7 Days to Die サーバーの TCP 26900 待受を確認できないため、完了マーカーを作成しません\n' >&2
    exit 1
  fi
  SERVER_STARTED=true
fi

# ゲームログを日次でローテーションし、直近14世代を保持する。
cat >/etc/logrotate.d/seven-days <<'ROTATE'
/srv/seven-days-data/logs/*.txt {
  daily
  rotate 14
  compress
  missingok
  notifempty
  copytruncate
}
ROTATE

# サーバーが active であることを確認できた場合だけ、次回の初期化処理を省略する。
if [ "$SERVER_STARTED" = true ]; then
  touch /var/lib/seven-days-provisioned
fi
