# 7 Days to Die サーバー Runbook

## 構成と初期構築

`infra/terraform/seven-days-server` を apply すると、東京リージョンの `e2-standard-4` VM、テスト用 10 GiB の分離 Persistent Disk、東京リージョンの GCS バックアップバケット、Cloud Billing export用BigQuery dataset、ゲーム用 firewall、Cloud Run 用 custom role、DuckDNS Secret の IAM、Budget Alert を作成する。外部 IPv4 は一時アドレスであり、停止中も Persistent Disk、GCS、BigQueryなどの料金は発生する。GCS バケットは既定で 30 日を過ぎたバックアップを自動削除する。継続運用では `data_disk_size_gb=20` 以上を推奨する。

1. Secret Manager に `SEVEN_DAYS_DUCKDNS_TOKEN` を登録する。ゲームサーバーと Telnet のパスワードは初回起動時に VM 内で生成するため、tfvars や state には入れない。
2. Terraform README に従い versioning 有効な state bucket を bootstrap し、必要なら `backup_bucket_name` を指定して apply する。バックアップ用バケットは Terraform が作成する。
   既存の 80 GiB データディスクは縮小できないため、既存環境では移行完了まで `data_disk_size_gb=80` を明示する。新しい 10 GiB ディスクへのコピーと短時間の起動確認を終えてから、ディスク参照を切り替える。継続運用へ移る場合は20GiB以上へ拡張する。
3. 初回起動時に startup script が `/srv/seven-days-data/serverconfig.xml` の `CHANGE_BEFORE_START` をランダム値へ置換する。ゲーム用と Telnet 用の値は、それぞれ root のみ読める `/srv/seven-days-data/server-password` と `/srv/seven-days-data/telnet-password` にも保存する。値を shell history やログへ出さない。
4. startup script がゲームサーバーを起動し、TCP 26900 の待受を確認してから Guest Attributes に起動時刻付きの READY を通知し、プロビジョニング完了とする。`serverconfig.xml` にプレースホルダーが残る場合は systemd の起動条件でも拒否する。起動後に service の状態と journal を確認し、以後は VM 起動時に自動起動する。
5. Terraform apply後、Cloud Billing ConsoleでStandard usage cost exportを有効化し、出力された `billing_export_dataset` を保存先に指定する。作成される `gcp_billing_export_v1_...` table名を確認する。exportには反映遅延があり、初回backfill完了まで数日かかる場合がある。
6. Cloud Run に下記環境変数を設定し、DuckDNS token だけを Secret Manager から注入する。Discord の Guild、Channel、User、Role ID は環境変数へ入れず、起動後に DB へ登録する。

```text
SEVEN_DAYS_GCP_PROJECT, SEVEN_DAYS_GCP_ZONE, SEVEN_DAYS_GCP_INSTANCE
SEVEN_DAYS_DUCKDNS_DOMAIN, SEVEN_DAYS_DUCKDNS_TOKEN, SEVEN_DAYS_PORT
SEVEN_DAYS_BILLING_PROJECT, SEVEN_DAYS_BILLING_DATASET, SEVEN_DAYS_BILLING_TABLE
SEVEN_DAYS_BILLING_MAX_BYTES
```

Billing projectは未指定なら `SEVEN_DAYS_GCP_PROJECT`、1回のquery上限は未指定なら100 MB。datasetを指定しない場合は料金表示を無効化する。
7. migration 適用後、対象 Guild の `guild_member.is_admin` が `TRUE` のユーザーが `/7dtd setup channel:<channel>` を実行する。これにより Guild と操作 Channel が `seven_days_config`、実行者が管理者 User として `seven_days_operator` に登録される。既存の7DTD管理者も同じコマンドで Channel を変更できる。
8. 必要な User／Role を `/7dtd allow-user` または `/7dtd allow-role` で追加する。`admin:false` は status だけ、`admin:true` は start/status/stop と設定変更を許可する。不要になった許可は `/7dtd remove-user` または `/7dtd remove-role` で削除する。

初回 setup を行うユーザーに DB 管理者フラグがない場合は、対象 Guild と User を確認したうえで `guild_member.is_admin` を手動で `TRUE` にする。別 Guild の管理者フラグは bootstrap 権限として扱わない。設定未登録、無効設定、別 Channel、未登録 User／Role のコマンドは拒否する。

ゲームサーバーは身内の 2〜4 人だけで利用する。`game_source_ranges` には参加者の固定 IPv4 を `/32` で指定し、ゲーム用ポートへの接続元を限定する。VPN は利用しない。自宅回線の IP が変わった場合は、Terraform の値を更新して再 apply する。SSH などの管理用ポートを全世界へ公開しない。

Cloud Run は Discord interaction を受ける Webhook として運用し、`--min-instances=0` を必須とする。`start` と `stop` は Compute Engine API へ要求を送った時点で応答し、起動時の READY 確認、DuckDNS 更新、停止完了と料金の確認は `/7dtd status` の実行時に行う。リクエスト後の処理継続を理由に最小インスタンス数を増やさない。

### 既存VMへのREADY通知移行

Guest Attributes導入前に構築済みのVMは、プロビジョニング完了マーカーにより通常のstartup scriptを省略するため、READY通知用ファイルを一度だけ移行する。移行スクリプトはTerraformで更新された安全停止処理もメタデータから同期する。先にCloud Run用custom roleへ`compute.instances.getGuestAttributes`を追加し、VMメタデータへ`enable-guest-attributes=TRUE`を設定する。

`google_compute_instance.server`のTerraform planにVM置換が含まれる場合はapplyしない。既存データディスクを保持したまま移行するため、`migrate-runtime-state.sh`を一時的なstartup scriptとして設定し、通常停止・起動で配置した後、元の`install.sh`へ戻す。移行用startup scriptは稼働中のunitをreloadしないため、新しいunitを読み込ませる目的でもう一度通常停止・起動する。

```sh
SEVEN_DAYS_PROJECT=project-d7a8d346-0d55-468c-ace
SEVEN_DAYS_ZONE=asia-northeast1-b
SEVEN_DAYS_INSTANCE=zunda-7dtd

gcloud compute instances add-metadata "$SEVEN_DAYS_INSTANCE" \
  --project="$SEVEN_DAYS_PROJECT" --zone="$SEVEN_DAYS_ZONE" \
  --metadata=enable-guest-attributes=TRUE \
  --metadata-from-file=startup-script=infra/terraform/seven-days-server/migrate-runtime-state.sh

# 対象名・zone・状態を確認してから通常停止・起動する。
gcloud compute instances describe "$SEVEN_DAYS_INSTANCE" \
  --project="$SEVEN_DAYS_PROJECT" --zone="$SEVEN_DAYS_ZONE" \
  --format='value(name,status,zone)'
gcloud compute instances stop "$SEVEN_DAYS_INSTANCE" \
  --project="$SEVEN_DAYS_PROJECT" --zone="$SEVEN_DAYS_ZONE"
gcloud compute instances start "$SEVEN_DAYS_INSTANCE" \
  --project="$SEVEN_DAYS_PROJECT" --zone="$SEVEN_DAYS_ZONE"

# serial logで migration completed を確認後、通常のstartup scriptへ戻す。
gcloud compute instances add-metadata "$SEVEN_DAYS_INSTANCE" \
  --project="$SEVEN_DAYS_PROJECT" --zone="$SEVEN_DAYS_ZONE" \
  --metadata-from-file=startup-script=infra/seven-days/install.sh

gcloud compute instances describe "$SEVEN_DAYS_INSTANCE" \
  --project="$SEVEN_DAYS_PROJECT" --zone="$SEVEN_DAYS_ZONE" \
  --format='value(name,status,zone)'
gcloud compute instances stop "$SEVEN_DAYS_INSTANCE" \
  --project="$SEVEN_DAYS_PROJECT" --zone="$SEVEN_DAYS_ZONE"
gcloud compute instances start "$SEVEN_DAYS_INSTANCE" \
  --project="$SEVEN_DAYS_PROJECT" --zone="$SEVEN_DAYS_ZONE"

gcloud compute instances get-guest-attributes "$SEVEN_DAYS_INSTANCE" \
  --project="$SEVEN_DAYS_PROJECT" --zone="$SEVEN_DAYS_ZONE" \
  --query-path=seven-days/runtime-state
```

最後に`/7dtd status`でREADY、現在の外部IPv4、DuckDNSの同期を確認する。移行中にVMの置換、ディスクの削除、強制停止は行わない。

## Windows セーブ移行

1. クライアントと dedicated server のバージョンを一致させ、両方を停止する。元データはコピーして原本を保管する。
2. `%APPDATA%\7DaysToDie\Saves` を `/srv/seven-days-data/Saves`、`GeneratedWorlds` を同名ディレクトリへ転送する。必要なら `Mods` も転送する。セーブやバックアップは Git に追加しない。
3. 所有者を `seven-days:seven-days` にする。`serverconfig.xml` の `GameWorld` をワールドフォルダ名、`GameName` を Saves 内のセーブフォルダ名に合わせる。
4. サーバーを起動して建築、所持品、プレイヤーデータを確認する。同じワールドをローカルとサーバーで並行更新しない。

## 7DTD バージョンアップ

Stable の更新でもクライアント、セーブ、Mod との互換性を先に確認し、参加者がいない時間帯に実施する。更新先は Steam の branch 名（例: `v3.2.0`）で固定し、`public` の暗黙更新に任せない。更新前後の branch、build ID、バックアップの GCS オブジェクト名を運用記録へ残す。

以下は `zunda-7dtd` を v3.2.0 へ更新したときの手順である。プロジェクトなどは実環境に合わせる。

```sh
SEVEN_DAYS_PROJECT=project-d7a8d346-0d55-468c-ace
SEVEN_DAYS_ZONE=asia-northeast1-b
SEVEN_DAYS_INSTANCE=zunda-7dtd

gcloud compute instances describe "$SEVEN_DAYS_INSTANCE" \
  --project="$SEVEN_DAYS_PROJECT" \
  --zone="$SEVEN_DAYS_ZONE" \
  --format='yaml(name,status,networkInterfaces[0].accessConfigs[0].natIP)'

gcloud compute ssh root@"$SEVEN_DAYS_INSTANCE" \
  --project="$SEVEN_DAYS_PROJECT" \
  --zone="$SEVEN_DAYS_ZONE"
```

VM 内では、まず現在の build ID、起動ディスクの空き容量、Mod の有無を確認する。SteamCMD は更新ファイルを一時展開するため、空き容量が不足する場合は更新を開始せず boot disk を拡張する。

```sh
sed -n '/buildid/p' /opt/seven-days/steamapps/appmanifest_294420.acf
df -h /opt/seven-days
find /srv/seven-days-data/Mods -mindepth 1 -maxdepth 1 -printf '%f\n'
```

`seven-days.service` は異常終了時に再起動する。更新中の再起動を防ぐため、実行ファイルの実行権限だけを一時的に外してから既存の安全停止処理を呼ぶ。停止後は必ず権限を戻し、ゲームプロセスが存在しないことを確認してからバックアップする。途中で想定外のプロセスが表示された場合は更新を続けない。

```sh
chmod 0644 /opt/seven-days/7DaysToDieServer.x86_64
/usr/local/sbin/seven-days-safe-stop

ps -o pid=,lstart=,cmd= -C 7DaysToDieServer.x86_64
chmod 0755 /opt/seven-days/7DaysToDieServer.x86_64
stat -c '%a %U:%G %n' /opt/seven-days/7DaysToDieServer.x86_64

/usr/local/sbin/seven-days-backup
```

バックアップ成功時の `gs://.../backups/<timestamp>.tar.gz` を記録してから、専用ユーザーで対象 branch をインストール・検証する。パスワードや token はコマンドラインへ含めない。

```sh
SEVEN_DAYS_BRANCH=v3.2.0
runuser -u seven-days -- /usr/games/steamcmd \
  +force_install_dir /opt/seven-days \
  +login anonymous \
  +app_update 294420 -beta "$SEVEN_DAYS_BRANCH" validate \
  +quit

sed -n '/buildid/p' /opt/seven-days/steamapps/appmanifest_294420.acf
stat -c '%a %U:%G %n' /opt/seven-days/7DaysToDieServer.x86_64
exit
```

SteamCMD が成功し、期待する build IDと実行権限 `755`を確認できた場合だけVMを再起動する。停止・起動の直前には対象VMを再確認する。

```sh
gcloud compute instances describe "$SEVEN_DAYS_INSTANCE" \
  --project="$SEVEN_DAYS_PROJECT" --zone="$SEVEN_DAYS_ZONE" \
  --format='value(name,status,zone)'
gcloud compute instances stop "$SEVEN_DAYS_INSTANCE" \
  --project="$SEVEN_DAYS_PROJECT" --zone="$SEVEN_DAYS_ZONE"

gcloud compute instances describe "$SEVEN_DAYS_INSTANCE" \
  --project="$SEVEN_DAYS_PROJECT" --zone="$SEVEN_DAYS_ZONE" \
  --format='value(name,status,zone)'
gcloud compute instances start "$SEVEN_DAYS_INSTANCE" \
  --project="$SEVEN_DAYS_PROJECT" --zone="$SEVEN_DAYS_ZONE"
```

起動後はログの `INF Version`、ゲームプロセス、TCP 26900 の待受を確認する。Dedicated Server のGPU非搭載環境ではshader警告が出ることがあるため、警告の有無だけで失敗と判断せず、プロセスとポートを確認する。

```sh
gcloud compute ssh root@"$SEVEN_DAYS_INSTANCE" \
  --project="$SEVEN_DAYS_PROJECT" \
  --zone="$SEVEN_DAYS_ZONE" \
  --command="grep -h 'INF Version:' /opt/seven-days/output_log__*.txt | tail -1; ps -o pid=,etime=,cmd= -C 7DaysToDieServer.x86_64; ss -H -lnt 'sport = :26900'"
```

最後に Discord で `/7dtd start` を実行し、`/7dtd status` で現在の外部IPv4をDuckDNSへ同期して、許可済み参加端末からの接続を確認する。起動しない場合はVMを強制停止せず、更新時に記録したGCSバックアップと以前のSteam branchを使って切り戻す。

## 通常運用と障害対応

- `/7dtd setup channel:<channel>`: Guild 単位の DB 管理者または既存の7DTD管理者限定。操作 Channel を登録し、実行者を7DTD管理者にする。
- `/7dtd allow-user user:<user> admin:<bool>` / `/7dtd allow-role role:<role> admin:<bool>`: 7DTD Operator を追加・更新する。
- `/7dtd remove-user user:<user>` / `/7dtd remove-role role:<role>`: 7DTD Operator を削除する。
- `/7dtd start`: `admin:true` の Operator 限定。登録 Channel で実行し、TERMINATED の場合だけ起動を要求してREADYを待たずに応答する。以後は `/7dtd status` で確認する。
- `/7dtd status`: 登録済み Operator が登録 Channel で実行できる。VM、現在の起動に対応するゲームREADY、接続先、外部 IPv4、稼働時間を表示する。READY のときは外部 IPv4 を DuckDNS に反映する。停止済みの場合は Billing exportに反映済みの当月・当年net cost、通貨、集計反映時点も表示する。古い起動のREADYは採用しない。
- `/7dtd stop`: `admin:true` の Operator 限定。登録 Channel で Compute Engine の通常停止を要求し、完了を待たずに応答する。systemd `ExecStop` がゲーム内通知、`saveworld`、`shutdown`、プロセス終了確認を通常停止猶予内に行う。以後は `/7dtd status` で停止完了を確認する。
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
