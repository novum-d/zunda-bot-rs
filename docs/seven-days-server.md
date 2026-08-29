# 7 Days to Die サーバー Runbook

## 構成と初期構築

`infra/terraform/seven-days-server` を apply すると、東京リージョンの `e2-standard-4` VM、テスト用 10 GiB の分離 Persistent Disk、東京リージョンの GCS バックアップバケット、ゲーム用 firewall、Cloud Run 用 custom role、DuckDNS Secret の IAM、Budget Alert を作成する。外部 IPv4 は一時アドレスであり、停止中も Persistent Disk と GCS の保存料金は発生する。GCS バケットは既定で 30 日を過ぎたバックアップを自動削除する。継続運用では `data_disk_size_gb=20` 以上を推奨する。

1. Secret Manager に `SEVEN_DAYS_DUCKDNS_TOKEN` を登録する。ゲームサーバーと Telnet のパスワードは初回起動時に VM 内で生成するため、tfvars や state には入れない。
2. Terraform README に従い versioning 有効な state bucket を bootstrap し、必要なら `backup_bucket_name` を指定して apply する。バックアップ用バケットは Terraform が作成する。
   既存の 80 GiB データディスクは縮小できないため、既存環境では移行完了まで `data_disk_size_gb=80` を明示する。新しい 10 GiB ディスクへのコピーと短時間の起動確認を終えてから、ディスク参照を切り替える。継続運用へ移る場合は20GiB以上へ拡張する。
3. 初回起動時に startup script が `/srv/seven-days-data/serverconfig.xml` の `CHANGE_BEFORE_START` をランダム値へ置換する。ゲーム用と Telnet 用の値は、それぞれ root のみ読める `/srv/seven-days-data/server-password` と `/srv/seven-days-data/telnet-password` にも保存する。値を shell history やログへ出さない。
4. startup script がゲームサーバーを起動し、TCP 26900 の待受を確認してからプロビジョニング完了とする。`serverconfig.xml` にプレースホルダーが残る場合は systemd の起動条件でも拒否する。起動後に service の状態と journal を確認し、以後は VM 起動時に自動起動する。
5. Cloud Run に下記環境変数を設定し、DuckDNS token だけを Secret Manager から注入する。Discord の Guild、Channel、User、Role ID は環境変数へ入れず、起動後に DB へ登録する。

```text
SEVEN_DAYS_GCP_PROJECT, SEVEN_DAYS_GCP_ZONE, SEVEN_DAYS_GCP_INSTANCE
SEVEN_DAYS_DUCKDNS_DOMAIN, SEVEN_DAYS_DUCKDNS_TOKEN, SEVEN_DAYS_PORT
```

6. migration 適用後、対象 Guild の `guild_member.is_admin` が `TRUE` のユーザーが `/7dtd setup channel:<channel>` を実行する。これにより Guild と操作 Channel が `seven_days_config`、実行者が管理者 User として `seven_days_operator` に登録される。既存の7DTD管理者も同じコマンドで Channel を変更できる。
7. 必要な User／Role を `/7dtd allow-user` または `/7dtd allow-role` で追加する。`admin:false` は status だけ、`admin:true` は start/status/stop と設定変更を許可する。不要になった許可は `/7dtd remove-user` または `/7dtd remove-role` で削除する。

初回 setup を行うユーザーに DB 管理者フラグがない場合は、対象 Guild と User を確認したうえで `guild_member.is_admin` を手動で `TRUE` にする。別 Guild の管理者フラグは bootstrap 権限として扱わない。設定未登録、無効設定、別 Channel、未登録 User／Role のコマンドは拒否する。

ゲームサーバーは身内の 2〜4 人だけで利用する。`game_source_ranges` には参加者の固定 IPv4 を `/32` で指定し、ゲーム用ポートへの接続元を限定する。VPN は利用しない。自宅回線の IP が変わった場合は、Terraform の値を更新して再 apply する。SSH などの管理用ポートを全世界へ公開しない。

`start` は Discord への defer 後も READY 確認を続ける。Cloud Tasks を使わない初期構成では処理中の instance 終了を避けるため、Cloud Run を `--min-instances=1 --no-cpu-throttling` に設定する。これは bot の待機費用を増やすため、将来 Cloud Tasks 化した時点で scale-to-zero に戻す。

## Windows セーブ移行

1. クライアントと dedicated server のバージョンを一致させ、両方を停止する。元データはコピーして原本を保管する。
2. `%APPDATA%\7DaysToDie\Saves` を `/srv/seven-days-data/Saves`、`GeneratedWorlds` を同名ディレクトリへ転送する。必要なら `Mods` も転送する。セーブやバックアップは Git に追加しない。
3. 所有者を `seven-days:seven-days` にする。`serverconfig.xml` の `GameWorld` をワールドフォルダ名、`GameName` を Saves 内のセーブフォルダ名に合わせる。
4. サーバーを起動して建築、所持品、プレイヤーデータを確認する。同じワールドをローカルとサーバーで並行更新しない。

## 通常運用と障害対応

- `/7dtd setup channel:<channel>`: Guild 単位の DB 管理者または既存の7DTD管理者限定。操作 Channel を登録し、実行者を7DTD管理者にする。
- `/7dtd allow-user user:<user> admin:<bool>` / `/7dtd allow-role role:<role> admin:<bool>`: 7DTD Operator を追加・更新する。
- `/7dtd remove-user user:<user>` / `/7dtd remove-role role:<role>`: 7DTD Operator を削除する。
- `/7dtd start`: `admin:true` の Operator 限定。登録 Channel で実行し、TERMINATED の場合だけ起動して外部 IPv4 を DuckDNS に登録し、TCP 26900 が開くまで待つ。
- `/7dtd status`: 登録済み Operator が登録 Channel で実行できる。VM、ポート、domain、外部 IPv4、稼働時間を表示する。
- `/7dtd stop`: `admin:true` の Operator 限定。登録 Channel で Compute Engine の通常停止を要求する。systemd `ExecStop` がゲーム内通知、`saveworld`、`shutdown`、プロセス終了確認を行う。停止完了は status で確認する。
- `/7dtd start` と `/7dtd stop` はPostgreSQLのトランザクションロックを取得してからVM操作を行う。別の開始・停止処理が実行中ならVM APIを呼ばず使用中メッセージを返す。Cloud Runが複数インスタンスでも同じDBロックを共有する。
- READY にならない場合は serial/startup logs、`systemctl status seven-days`、`journalctl -u seven-days`、firewall、`serverconfig.xml` を確認する。
- DuckDNS 失敗時は Secret の version と Cloud Run service account の accessor IAM を確認する。token を URL やログに貼らない。
- stop が完了しない場合は VM を強制停止せず journal と telnet password file を確認し、ゲーム内で保存後に再試行する。

## Backup と復元確認

### バックアップ要件

- ゲームサーバーが稼働していない状態で、ゲーム内保存後に `/usr/local/sbin/seven-days-backup` を手動実行する。
- `/usr/local/sbin/seven-days-backup` は Saves、GeneratedWorlds、serverconfig.xml、存在する場合は Mods を gzip 圧縮し、Terraform が作成した GCS バケットの `backups/` へアップロードする。
- アップロード中だけデータディスク上に一時アーカイブを作成し、成功・失敗を問わず終了時に削除する。
- GCS バケットのライフサイクルにより、既定では 30 日を過ぎたバックアップを削除する。長期保管が必要な場合は `backup_retention_days` を変更する。
- 強制停止やクラッシュ時のバックアップは保証しない。必要な場合は、別途手動で復旧手順を実行する。

復元は GCS の対象オブジェクトを検証用 VM へダウンロードして展開し、Saves/GeneratedWorlds と起動を確認する。確認後に本番 VM を停止し、必要なデータだけを本番ディスクへ戻す。少なくとも初回構築後に一度この復元テストを行い、GCS オブジェクト名と結果を運用記録へ残す。
