#!/usr/bin/env bash
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive

if [ -f /var/lib/seven-days-provisioned ]; then exit 0; fi

apt-get update
apt-get install -y software-properties-common
add-apt-repository -y multiverse
dpkg --add-architecture i386
apt-get update
printf 'steam steam/question select I AGREE\nsteam steam/license note \n' | debconf-set-selections
apt-get install -y steamcmd xfsprogs telnet logrotate
id seven-days >/dev/null 2>&1 || useradd --system --create-home --shell /usr/sbin/nologin seven-days

DEVICE=/dev/disk/by-id/google-seven-days-data
MOUNT=/srv/seven-days-data
mkdir -p "$MOUNT"
if ! blkid "$DEVICE" >/dev/null 2>&1; then mkfs.xfs "$DEVICE"; fi
UUID=$(blkid -s UUID -o value "$DEVICE")
grep -q "$UUID" /etc/fstab || printf 'UUID=%s %s xfs defaults,nofail 0 2\n' "$UUID" "$MOUNT" >> /etc/fstab
mount "$MOUNT" || mount -a
mkdir -p "$MOUNT"/{Saves,GeneratedWorlds,Mods,backups} /opt/seven-days
chown -R seven-days:seven-days "$MOUNT" /opt/seven-days

runuser -u seven-days -- steamcmd +force_install_dir /opt/seven-days +login anonymous +app_update 294420 validate +quit
METADATA=http://metadata.google.internal/computeMetadata/v1/instance/attributes
metadata_file() { curl -fsS -H 'Metadata-Flavor: Google' "$METADATA/$1" | base64 -d >"$2"; }
metadata_file seven-days-safe-stop /usr/local/sbin/seven-days-safe-stop
metadata_file seven-days-backup /usr/local/sbin/seven-days-backup
chmod 0755 /usr/local/sbin/seven-days-safe-stop /usr/local/sbin/seven-days-backup
if [ ! -f "$MOUNT/serverconfig.xml" ]; then metadata_file seven-days-config "$MOUNT/serverconfig.xml"; fi

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

systemctl daemon-reload
systemctl enable seven-days.service
if ! grep -q 'CHANGE_BEFORE_START' "$MOUNT/serverconfig.xml"; then systemctl start seven-days.service; fi
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
touch /var/lib/seven-days-provisioned
