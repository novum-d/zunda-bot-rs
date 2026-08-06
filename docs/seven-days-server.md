# 7 Days to Die サーバー Runbook

## 構成と初期構築

`infra/terraform/seven-days-server` を apply すると、東京リージョンの `e2-standard-4` VM、80 GB の分離 Persistent Disk、ゲーム用 firewall、日次 snapshot policy、Cloud Run 用 custom role、DuckDNS Secret の IAM、Budget Alert を作成する。外部 IPv4 は一時アドレスであり、停止中も Persistent Disk、snapshot 等の料金は発生する。日次 snapshot policy は現行 Terraform の実装であり、計画ではゲーム正常停止後の明示的なスナップショットへ置き換える。

1. Secret Manager に `SEVEN_DAYS_DUCKDNS_TOKEN` とサーバーパスワードを登録する。値を tfvars や state に入れない。
2. Terraform README に従い versioning 有効な state bucket を bootstrap し apply する。
3. 初回 SSH で `/srv/seven-days-data/serverconfig.xml` の `CHANGE_BEFORE_START` を Secret Manager から安全に取得した値へ置換する。Telnet password と同じ値を root のみ読める `/srv/seven-days-data/telnet-password` に保存する（`ExecStop=+` の停止処理だけが読む）。値を shell history やログへ出さない。
4. `systemctl start seven-days` を実行し、`systemctl status seven-days` と `journalctl -u seven-days` で確認する。以後は VM 起動時に自動起動する。
5. Cloud Run に下記環境変数を設定し、DuckDNS token だけを Secret Manager から注入する。

```text
SEVEN_DAYS_GCP_PROJECT, SEVEN_DAYS_GCP_ZONE, SEVEN_DAYS_GCP_INSTANCE
SEVEN_DAYS_DUCKDNS_DOMAIN, SEVEN_DAYS_DUCKDNS_TOKEN, SEVEN_DAYS_PORT
SEVEN_DAYS_DISCORD_GUILD_ID, SEVEN_DAYS_DISCORD_CHANNEL_ID
SEVEN_DAYS_DISCORD_USER_IDS, SEVEN_DAYS_DISCORD_ROLE_IDS
SEVEN_DAYS_DISCORD_ADMIN_USER_IDS, SEVEN_DAYS_DISCORD_ADMIN_ROLE_IDS
```

ID のリストはカンマ区切り。一般 ID は start/status、管理 ID は start/status/stop を実行できる。

ゲームサーバーは身内の 2〜4 人だけで利用する。`game_source_ranges` には参加者の固定 IPv4 を `/32` で指定し、ゲーム用ポートへの接続元を限定する。VPN は利用しない。自宅回線の IP が変わった場合は、Terraform の値を更新して再 apply する。SSH などの管理用ポートを全世界へ公開しない。

`start` は Discord への defer 後も READY 確認を続ける。Cloud Tasks を使わない初期構成では処理中の instance 終了を避けるため、Cloud Run を `--min-instances=1 --no-cpu-throttling` に設定する。これは bot の待機費用を増やすため、将来 Cloud Tasks 化した時点で scale-to-zero に戻す。

## Windows セーブ移行

1. クライアントと dedicated server のバージョンを一致させ、両方を停止する。元データはコピーして原本を保管する。
2. `%APPDATA%\7DaysToDie\Saves` を `/srv/seven-days-data/Saves`、`GeneratedWorlds` を同名ディレクトリへ転送する。必要なら `Mods` も転送する。セーブやバックアップは Git に追加しない。
3. 所有者を `seven-days:seven-days` にする。`serverconfig.xml` の `GameWorld` をワールドフォルダ名、`GameName` を Saves 内のセーブフォルダ名に合わせる。
4. サーバーを起動して建築、所持品、プレイヤーデータを確認する。同じワールドをローカルとサーバーで並行更新しない。

## 通常運用と障害対応

- `/7dtd start`: TERMINATED の場合だけ起動し、外部 IPv4 を DuckDNS に登録して TCP 26900 が開くまで待つ。
- `/7dtd status`: VM、ポート、domain、外部 IPv4、稼働時間を表示する。
- `/7dtd stop`: Compute Engine の通常停止を要求する。systemd `ExecStop` がゲーム内通知、`saveworld`、`shutdown`、プロセス終了確認を行う。停止完了は status で確認する。計画では、ゲーム内保存と正常停止の完了後にスナップショットを作成する。
- READY にならない場合は serial/startup logs、`systemctl status seven-days`、`journalctl -u seven-days`、firewall、`serverconfig.xml` を確認する。
- DuckDNS 失敗時は Secret の version と Cloud Run service account の accessor IAM を確認する。token を URL やログに貼らない。
- stop が完了しない場合は VM を強制停止せず journal と telnet password file を確認し、ゲーム内で保存後に再試行する。

## Backup と復元確認

### バックアップ要件

- ゲームサーバーが稼働していない状態で、自動バックアップを実行しない。
- ゲーム終了時は、ゲーム内保存と正常停止が完了した後にだけスナップショットを作成する。
- 定期実行の snapshot policy は使用せず、`/7dtd stop` の停止フローから明示的にスナップショットを作成する。
- 強制停止やクラッシュ時のバックアップは保証しない。必要な場合は、別途手動で復旧手順を実行する。

上記は運用要件であり、現時点の Terraform にある日次 snapshot policy は未対応である。

`/usr/local/sbin/seven-days-backup` は data disk 内の `backups/<UTC時刻>` に手動コピーを作る。実行前にゲーム内保存する。

復元は新しい disk を snapshot から作り、検証用 VM に read/write attach して Saves/GeneratedWorlds と起動を確認する。確認後に本番 VM を停止し、Terraform の disk 参照を復元 disk に計画的に切り替える。元 disk は検証完了まで削除しない。少なくとも初回構築後に一度この復元テストを行い、snapshot 名と結果を運用記録へ残す。
