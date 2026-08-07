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
runuser -u seven-days -- steamcmd +force_install_dir /opt/seven-days +login anonymous +app_update 294420 validate +quit

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
ExecStart=/opt/seven-days/startserver.sh -configfile=/srv/seven-days-data/serverconfig.xml -UserDataFolder=/srv/seven-days-data
ExecStop=+/usr/local/sbin/seven-days-safe-stop
TimeoutStopSec=180
Restart=on-failure

[Install]
WantedBy=multi-user.target
UNIT

# unit を登録して自動起動を有効化する。
systemctl daemon-reload
systemctl enable seven-days.service

# プレースホルダーが残る場合は起動せず、設定変更後の再実行に備える。
SERVER_STARTED=false
if grep -q 'CHANGE_BEFORE_START' "$MOUNT/serverconfig.xml"; then
  printf 'serverconfig.xml の CHANGE_BEFORE_START を変更してから再実行してください\n' >&2
else
  systemctl start seven-days.service
  # start の成功だけでなく、systemd が active と判定したことも確認する。
  if ! systemctl is-active --quiet seven-days.service; then
    printf '7 Days to Die サーバーの起動を確認できないため、完了マーカーを作成しません\n' >&2
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
