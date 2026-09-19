#!/usr/bin/env bash
# READY 通知導入前に構築済みの VM へ Guest Attributes 連携を後付けする一回限りの startup script。
set -euo pipefail

if [ ! -f /var/lib/seven-days-provisioned ]; then
  printf '既存の7DTDプロビジョニング完了マーカーがないため移行を中止します\n' >&2
  exit 1
fi

for required_path in \
  /opt/seven-days/startserver.sh \
  /srv/seven-days-data/serverconfig.xml \
  /usr/local/sbin/seven-days-safe-stop; do
  if [ ! -e "$required_path" ]; then
    printf '移行に必要なファイルがありません: %s\n' "$required_path" >&2
    exit 1
  fi
done

# Terraformで更新した安全停止処理も既存VMへ同期する。
curl -fsS -H 'Metadata-Flavor: Google' \
  http://metadata.google.internal/computeMetadata/v1/instance/attributes/seven-days-safe-stop \
  | base64 -d >/usr/local/sbin/seven-days-safe-stop
chmod 0755 /usr/local/sbin/seven-days-safe-stop

cat >/usr/local/sbin/seven-days-state <<'STATE'
#!/usr/bin/env bash
set -euo pipefail
STATE=${1:?state is required}
STARTED_AT_FILE=/run/seven-days-started-at
if [ "$STATE" = STARTING ]; then date -u +%Y-%m-%dT%H:%M:%SZ >"$STARTED_AT_FILE"; fi
STARTED_AT=$(<"$STARTED_AT_FILE")
BOOT_ID=$(</proc/sys/kernel/random/boot_id)
curl -fsS -X PUT \
  -H 'Metadata-Flavor: Google' \
  --data "$STATE|$BOOT_ID|$STARTED_AT" \
  http://metadata.google.internal/computeMetadata/v1/instance/guest-attributes/seven-days/runtime-state \
  >/dev/null
STATE

cat >/usr/local/sbin/seven-days-ready <<'READY'
#!/usr/bin/env bash
set -euo pipefail
for _ in $(seq 1 120); do
  if ss -H -lnt 'sport = :26900' | grep -q .; then
    /usr/local/sbin/seven-days-state READY
    exit 0
  fi
  sleep 5
done
/usr/local/sbin/seven-days-state FAILED
exit 1
READY
chmod 0755 /usr/local/sbin/seven-days-state /usr/local/sbin/seven-days-ready

# daemon-reload は行わず、次回の通常起動時に systemd がこの unit を読み込む。
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
ExecStartPre=+/usr/local/sbin/seven-days-state STARTING
ExecStart=/opt/seven-days/startserver.sh -configfile=/srv/seven-days-data/serverconfig.xml -UserDataFolder=/srv/seven-days-data
ExecStartPost=+/usr/local/sbin/seven-days-ready
ExecStop=+/usr/local/sbin/seven-days-safe-stop
ExecStopPost=+/usr/local/sbin/seven-days-state STOPPED
TimeoutStartSec=620
TimeoutStopSec=110
Restart=on-failure

[Install]
WantedBy=multi-user.target
UNIT

printf 'seven-days runtime-state migration completed\n'
